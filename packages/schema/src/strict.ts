import { Type, type TProperties, type TSchemaOptions } from "typebox";
/** Closed wire objects; annotations and bounds remain at their owning schema. */
export const Strict = <P extends TProperties>(properties: P, options: TSchemaOptions = {}) =>
  Type.Object(properties, { ...options, additionalProperties: false });
