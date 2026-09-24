import 'package:aidog_flutter/src/deep_link.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('parses aidog entity, action, and data', () {
    expect(
      parseDeepLink(
        Uri.parse('aidog://skill/import?data=eyJza2lsbHMiOltdfQ=='),
      ),
      (entity: 'skill', action: 'import', data: 'eyJza2lsbHMiOltdfQ=='),
    );
  });

  test('defaults missing action and rejects other schemes', () {
    expect(parseDeepLink(Uri.parse('aidog://mcp?data=ew==')), (
      entity: 'mcp',
      action: 'import',
      data: 'ew==',
    ));
    expect(parseDeepLink(Uri.parse('https://example.com')), isNull);
  });

  test('dispatches each payload through pending and stream paths', () async {
    final bus = DeepLinkBus();
    final received = <DeepLinkPayload>[];
    final sub = bus.subscribe('skill', received.add);
    const payload = (entity: 'skill', action: 'import', data: 'x');
    bus.dispatch(payload);
    await Future<void>.delayed(Duration.zero);
    expect(received, [payload]);
    expect(bus.takePending('skill'), isNull);
    await sub.cancel();
    bus.dispose();
  });
}
