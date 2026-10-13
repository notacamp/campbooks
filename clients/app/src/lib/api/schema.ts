import type { components as adminComponents } from "./admin.gen";
import type { components } from "./schema.gen";

/**
 * A request or response type from the OpenAPI document, by schema name
 * (`Schema<"WorkspaceView">`). The generated file stays private to lib/api.
 */
export type Schema<TName extends keyof components["schemas"]> = components["schemas"][TName];

/** A type from the admin API's document (`AdminSchema<"AdminOverview">`). */
export type AdminSchema<TName extends keyof adminComponents["schemas"]> =
  adminComponents["schemas"][TName];
