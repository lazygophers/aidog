/** 固定脱敏 fixture，来源版本：aidog 0.1.11 文档演示基线。 */
export const demoModules = [
  'home',
  'platforms',
  'groups',
  'proxy',
  'logs',
  'stats',
  'settings',
  'notifications',
  'mcp',
  'skills',
  'claude-code',
  'codex',
] as const;

export type DemoModule = (typeof demoModules)[number];
export type DemoState = 'normal' | 'empty' | 'error' | 'loading';
export type DemoFixture = {
  title: string;
  eyebrow: string;
  summary: string;
  metrics: readonly { label: string; value: string }[];
  rows: readonly { name: string; detail: string; status: string }[];
};

const titles: Record<DemoModule, { title: string; eyebrow: string }> = {
  home: { title: '工作台概览', eyebrow: '今日运行状态' },
  platforms: { title: 'AI 平台', eyebrow: '平台连接' },
  groups: { title: '分组路由', eyebrow: '请求策略' },
  proxy: { title: '代理服务', eyebrow: '本地服务' },
  logs: { title: '代理日志', eyebrow: '请求记录' },
  stats: { title: '使用统计', eyebrow: '用量分析' },
  settings: { title: '系统设置', eyebrow: '偏好设置' },
  notifications: { title: '通知中心', eyebrow: '消息收件箱' },
  mcp: { title: 'MCP Server', eyebrow: '工具连接' },
  skills: { title: 'Skills', eyebrow: '可复用能力' },
  'claude-code': { title: 'Claude Code', eyebrow: '编程助手' },
  codex: { title: 'Codex', eyebrow: '编程助手' },
};

const rowNames: Record<DemoModule, string[]> = {
  home: ['代理服务', '今日请求'],
  platforms: ['Anthropic', 'OpenAI'],
  groups: ['默认分组', '编程助手'],
  proxy: ['代理监听', '请求超时'],
  logs: ['完成请求', '完成请求'],
  stats: ['Claude Sonnet 4', 'GPT-4.1'],
  settings: ['主题', '日志保留'],
  notifications: ['配置已同步', '余额提醒'],
  mcp: ['filesystem', 'github'],
  skills: ['代码审查', '文档搜索'],
  'claude-code': ['默认分组', 'StatusLine'],
  codex: ['aidog', '备用 provider'],
};

const normalRows = (module: DemoModule): DemoFixture['rows'] =>
  rowNames[module].map((name, i) => ({
    name,
    detail: ['固定脱敏配置', i ? '已配置' : '默认设置'][i],
    status: i ? '可用' : '启用',
  }));
const metrics = ['请求数', '输入 Token', '输出 Token'];

const createFixture = (module: DemoModule): Record<DemoState, DemoFixture> => {
  const heading = titles[module];
  return {
    normal: {
      ...heading,
      summary: '固定脱敏数据，仅用于文档演示。',
      metrics: metrics.map((label, i) => ({
        label,
        value: ['1,284', '1.8M', '438K'][i],
      })),
      rows: normalRows(module),
    },
    empty: {
      ...heading,
      summary: '当前没有可展示的数据。',
      metrics: metrics.map((label) => ({ label, value: '—' })),
      rows: [],
    },
    error: {
      ...heading,
      summary: '演示数据加载失败，请稍后重试。',
      metrics: metrics.map((label) => ({ label, value: '!' })),
      rows: [],
    },
    loading: {
      ...heading,
      summary: '正在读取固定演示数据……',
      metrics: metrics.map((label) => ({ label, value: '…' })),
      rows: [],
    },
  };
};

export const demoFixtures: Record<
  DemoModule,
  Record<DemoState, DemoFixture>
> = {
  home: createFixture('home'),
  platforms: createFixture('platforms'),
  groups: createFixture('groups'),
  proxy: createFixture('proxy'),
  logs: createFixture('logs'),
  stats: createFixture('stats'),
  settings: createFixture('settings'),
  notifications: createFixture('notifications'),
  mcp: createFixture('mcp'),
  skills: createFixture('skills'),
  'claude-code': createFixture('claude-code'),
  codex: createFixture('codex'),
};

export const stateLabels: Record<DemoState, string> = {
  normal: '正常',
  empty: '空状态',
  error: '错误',
  loading: '加载中',
};
