export {
  registerExtensions,
  registerGateResolver,
  getExtensions,
  isExtensionEnabled,
} from "./registry";
export type {
  Extension,
  AnyExtension,
  ExtensionPointContracts,
  ExtensionPointName,
} from "./registry";
export { ExtensionPoint } from "./extension-point";
