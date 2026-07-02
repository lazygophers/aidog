// Barrel re-export — keeps `components/settings/editors` import path stable.
// Source split happened in arch-redesign phase 3 (editors.tsx was 4621 lines).

export { F, S } from "./tokens";
export { SvgIcon, SectionIcon, ICON_PATHS } from "./icons";
export {
  Toggle,
  Section,
  Highlighted,
  FieldLabel,
  JsonEditor,
  KvEditor,
  StringListEditor,
  FieldRow,
  PathInput,
  type PathSuggestion,
} from "./_shared";
export { EnvEditor } from "./EnvEditor";
export { PermissionsSection, PermissionsSectionInline } from "./PermissionsSection";
export { SandboxSection, SandboxSectionInline } from "./SandboxSection";
export { StatusLineSection } from "./StatusLineSection";
export {
  type DiffNode,
  isPlainObject,
  readManagedPaths,
  buildImportDiffTree,
  ImportDiffModal,
} from "./ImportDiff";
export { PluginsSection, PluginsSectionInline } from "./PluginsSection";
export {
  type HooksConfig,
  type HookHandler,
  type MatcherGroup,
  type HandlerType,
  HOOK_EVENTS,
  HANDLER_TYPES,
  HANDLER_LABELS,
  NotifyHookQuickBar,
} from "./hooks-types";
export { HooksSection } from "./HooksSection";
export { HooksSectionInline } from "./HooksSectionInline";
export { FieldRenderer } from "./FieldRenderer";
