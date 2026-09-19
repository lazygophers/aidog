// 内核子进程的生命周期（spec 阶段 1）。
//
// `aidog-kernel --ui --port 0` 由系统挑一个空闲端口，并把实际监听地址以
// `AIDOG_KERNEL_LISTEN=<url>` 一行打到标准输出（`aidog_kernel/src/main.rs:47`）。
// `--port 0` 下除了让内核自己报，调用方没有别的途径知道端口是几。
//
// 内核自带单实例锁（按 data_dir 生效）。已经有一个在跑时，新起的这个会打印已有地址再
// `exit 0` —— 那种情况我们照样拿到了地址，只是地址属于别人的进程，不该去重拉。

import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// 内核连接状态。UI 直接拿它渲染「后端连接中 / 已连接 / 重连中」。
enum KernelState {
  /// 正在拉起内核、还没拿到监听地址。
  connecting,

  /// 已拿到地址，可以打 RPC。
  connected,

  /// 之前连上过，内核挂了，正在退避重拉。
  reconnecting,

  /// 调用方主动停了，不会再自动重拉。
  stopped,
}

/// `aidog-kernel` 打在标准输出上的地址行前缀（`main.rs::LISTEN_LINE_PREFIX`）。
const String kListenLinePrefix = 'AIDOG_KERNEL_LISTEN=';

/// 指数退避：1s / 2s / 4s …，上限 30s（票 I01）。
const Duration _backoffInitial = Duration(seconds: 1);
const Duration _backoffMax = Duration(seconds: 30);

class KernelProcess {
  KernelProcess({
    String? executable,
    this.extraArgs = const <String>[],
    this.environment,
    this.startupTimeout = const Duration(seconds: 30),
  }) : executable = executable ?? resolveKernelExecutable();

  /// 内核可执行文件路径。
  final String executable;

  /// 附加参数（`--ui --port 0` 由本类自己加）。
  final List<String> extraArgs;

  /// 子进程环境变量覆盖。测试用它把 `HOME` 指到隔离目录，**避免碰用户的 `~/.aidog`**。
  final Map<String, String>? environment;

  /// 拉起后多久没等到地址行就算失败。
  final Duration startupTimeout;

  final StreamController<KernelState> _states =
      StreamController<KernelState>.broadcast();

  /// 内核日志行（stdout + stderr），供诊断面板消费。丢弃无订阅者时的历史。
  final StreamController<String> _logs = StreamController<String>.broadcast();

  /// 每次拿到监听地址都推一条。**重拉后端口会变**（`--port 0`），上层必须据此重建连接。
  final StreamController<Uri> _addresses = StreamController<Uri>.broadcast();

  Process? _process;
  Uri? _address;
  KernelState _state = KernelState.stopped;
  Duration _backoff = _backoffInitial;
  bool _started = false;
  bool _stopRequested = false;

  /// 「当前地址什么时候可用」。重拉期间换成一个新的未完成 completer，所以 [start] 在重连中
  /// 返回的是**新**地址而不是上一次的回声 —— `--port 0` 每次起来端口都不同。
  Completer<Uri> _ready = Completer<Uri>();

  /// 已经有别的内核实例占着 data_dir —— 我们拉起的这个打印完地址就 exit 0 了。
  /// 这种情况下地址有效但进程不是我们的，不重拉。
  bool _adoptedExternal = false;

  KernelState get state => _state;
  Stream<KernelState> get states => _states.stream;
  Stream<String> get logs => _logs.stream;
  Stream<Uri> get addresses => _addresses.stream;

  /// 当前监听地址，未连上时为 `null`。
  Uri? get address => _address;

  /// 内核子进程 pid，没有在跑时为 `null`。诊断与测试用。
  int? get pid => _process?.pid;

  /// 拉起内核并等到地址可用。重复调用无副作用；重连中调用会等到**新**地址。
  Future<Uri> start() {
    if (!_started) {
      _started = true;
      _stopRequested = false;
      _setState(KernelState.connecting);
      unawaited(_spawn());
    }
    return _ready.future;
  }

  Future<void> _spawn() async {
    try {
      final proc = await Process.start(
        executable,
        <String>['--ui', '--port', '0', ...extraArgs],
        environment: environment,
        // 不继承父进程环境时测试无法拿到 PATH 等；这里始终继承，只叠加覆盖。
        includeParentEnvironment: true,
      );
      _process = proc;

      var sawListenLine = false;
      proc.stdout.transform(utf8.decoder).transform(const LineSplitter()).listen(
        (line) {
          _emitLog(line);
          if (!line.startsWith(kListenLinePrefix)) return;
          sawListenLine = true;
          final url = line.substring(kListenLinePrefix.length).trim();
          _onAddress(Uri.parse(url));
        },
        onError: _emitLog,
      );
      proc.stderr
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(_emitLog, onError: _emitLog);

      unawaited(
        proc.exitCode.then((code) {
          if (identical(_process, proc)) _process = null;
          _emitLog('aidog-kernel exited with code $code');
          if (_stopRequested) return;
          // 抢不到单实例锁的那条路：打印完已有实例的地址就 exit 0。地址是真的，
          // 进程不是我们的 —— 别去重拉，重拉只会再 exit 0 一次，无限循环。
          if (code == 0 && sawListenLine && _address != null) {
            _adoptedExternal = true;
            _setState(KernelState.connected);
            return;
          }
          _scheduleRetry('kernel exited with code $code');
        }),
      );

      // 起不来也得让等的人知道，不能挂死。
      unawaited(
        Future<void>.delayed(startupTimeout).then((_) {
          if (_address == null && identical(_process, proc)) {
            proc.kill(ProcessSignal.sigkill);
            _scheduleRetry('no $kListenLinePrefix line within $startupTimeout');
          }
        }),
      );
    } catch (e) {
      _scheduleRetry('cannot spawn $executable: $e');
    }
  }

  void _onAddress(Uri url) {
    _address = url;
    _backoff = _backoffInitial;
    if (!_ready.isCompleted) _ready.complete(url);
    if (!_addresses.isClosed) _addresses.add(url);
    _setState(KernelState.connected);
  }

  void _scheduleRetry(String reason) {
    if (_stopRequested || _adoptedExternal) return;
    _emitLog('kernel: $reason, retrying in ${_backoff.inSeconds}s');
    _address = null;
    if (_ready.isCompleted) _ready = Completer<Uri>();
    _setState(KernelState.reconnecting);
    final delay = _backoff;
    _backoff = delay * 2 > _backoffMax ? _backoffMax : delay * 2;
    Timer(delay, () {
      if (_stopRequested) return;
      unawaited(_spawn());
    });
  }

  void _setState(KernelState s) {
    if (_state == s) return;
    _state = s;
    if (!_states.isClosed) _states.add(s);
  }

  void _emitLog(Object line) {
    if (!_logs.isClosed) _logs.add(line.toString());
  }

  /// 停掉内核并放弃自动重拉。
  Future<void> stop() async {
    _stopRequested = true;
    _setState(KernelState.stopped);
    final p = _process;
    _process = null;
    _address = null;
    _started = false;
    if (!_ready.isCompleted) {
      _ready.completeError(StateError('kernel stopped before it was ready'));
    }
    if (p != null) {
      p.kill(ProcessSignal.sigterm);
      await p.exitCode.timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          p.kill(ProcessSignal.sigkill);
          return -1;
        },
      );
    }
    await _states.close();
    await _logs.close();
    await _addresses.close();
  }
}

/// 找 `aidog-kernel` 可执行文件。三条路都要能走通：
///
/// 1. `AIDOG_KERNEL_BIN` 环境变量 —— 测试与特殊部署用。
/// 2. **发版形态**：与 Flutter 可执行文件同目录（macOS 的 `Foo.app/Contents/MacOS/`，
///    Windows 的安装目录，见 spec §5.2）。
/// 3. **开发形态**：从当前目录逐级向上找 `src-tauri/target/{release,debug}/aidog-kernel`。
///
/// 一条都不命中就抛 —— 静默回落到某个猜出来的路径只会让错误出现在更远的地方。
String resolveKernelExecutable({String? startDir}) {
  final name = Platform.isWindows ? 'aidog-kernel.exe' : 'aidog-kernel';

  final fromEnv = Platform.environment['AIDOG_KERNEL_BIN'];
  if (fromEnv != null && fromEnv.isNotEmpty) return fromEnv;

  final beside = File(
    '${File(Platform.resolvedExecutable).parent.path}${Platform.pathSeparator}$name',
  );
  if (beside.existsSync()) return beside.path;

  var dir = Directory(startDir ?? Directory.current.path).absolute;
  while (true) {
    // release 与 debug 都在时取**改得更晚**的那个，不按固定优先级。固定偏好 release 会在
    // 「刚 `cargo build` 了 debug、release 还是上周的」时静默用旧二进制 —— 真踩过：旧 release
    // 不认 `--port`，表现成「内核起不来」，离真正的原因隔了两层。
    final found = <File>[];
    for (final profile in const <String>['release', 'debug']) {
      final f = File(
        <String>[
          dir.path,
          'src-tauri',
          'target',
          profile,
          name,
        ].join(Platform.pathSeparator),
      );
      if (f.existsSync()) found.add(f);
    }
    if (found.isNotEmpty) {
      found.sort(
        (a, b) => b.lastModifiedSync().compareTo(a.lastModifiedSync()),
      );
      return found.first.path;
    }
    final parent = dir.parent;
    if (parent.path == dir.path) break;
    dir = parent;
  }

  throw StateError(
    'aidog-kernel not found. Set AIDOG_KERNEL_BIN, put it next to '
    '${Platform.resolvedExecutable}, or build it with '
    '`cargo build -p aidog_kernel` under src-tauri/.',
  );
}
