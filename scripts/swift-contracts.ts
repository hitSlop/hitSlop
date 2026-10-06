import type { TSchema } from "typebox";

// A deliberately bounded emitter for the platform envelopes, not an app-schema compiler.
type Schema = {
  type?: string;
  const?: unknown;
  enum?: unknown[];
  anyOf?: Schema[];
  properties?: Record<string, Schema>;
  required?: string[];
  items?: Schema;
  additionalProperties?: boolean | Schema;
  patternProperties?: Record<string, Schema>;
  [key: string]: unknown;
};
const quote = JSON.stringify;
const title = (value: string) =>
  value.replace(/(^|[._-])([a-z])/g, (_, _separator, letter) => letter.toUpperCase());
const identifier = (value: string) => {
  const name = title(value);
  return "`" + name[0]!.toLowerCase() + name.slice(1) + "`";
};
function unsupported(path: string): never {
  throw new Error(`Unsupported Swift contract schema at ${path}`);
}

export function swiftContracts(
  request: TSchema,
  reply: TSchema,
  discovery: TSchema,
  pageResults: Record<string, TSchema>,
) {
  const declarations: string[] = [];
  function checkKeys(schema: Schema, path: string) {
    const allowed = new Set([
      "type",
      "const",
      "enum",
      "properties",
      "required",
      "items",
      "additionalProperties",
      "patternProperties",
      "minLength",
      "maxLength",
      "pattern",
      "minimum",
      "maximum",
      "minItems",
      "maxItems",
      "maxProperties",
      "uniqueItems",
      "description",
      "title",
    ]);
    if (Object.keys(schema).some((key) => !allowed.has(key))) unsupported(path);
  }
  function enumeration(name: string, values: unknown[], path: string) {
    if (
      !values.length ||
      values.some((v) => typeof v !== "string" || !/^[a-zA-Z][a-zA-Z0-9._-]*$/.test(v))
    )
      unsupported(path);
    if (new Set(values.map((v) => identifier(v as string))).size !== values.length)
      unsupported(path);
    declarations.push(
      `public enum ${name}: String, CaseIterable, Sendable {\n${values.map((v) => `  case ${identifier(v as string)} = ${quote(v)}`).join("\n")}\n}`,
    );
    return name;
  }
  function fieldType(schema: Schema, name: string, path: string): { type: string; enum: boolean } {
    // A union of closed objects (the manifest presentation) crosses as JSON; its owner
    // validates the members.
    if (schema.anyOf) {
      if (!schema.anyOf.every((member) => member.type === "object")) unsupported(path);
      return { type: "[String: Any]", enum: false };
    }
    // Unknown annotations must not hide an unsupported structural construct.
    checkKeys(schema, path);
    if (schema.enum) return { type: enumeration(name, schema.enum, path), enum: true };
    if (schema.const !== undefined)
      return { type: enumeration(name, [schema.const], path), enum: true };
    if (Object.keys(schema).length === 0) return { type: "Any", enum: false };
    if (schema.type === "string") return { type: "String", enum: false };
    if (schema.type === "integer") return { type: "Int", enum: false };
    if (schema.type === "number") return { type: "Double", enum: false };
    if (schema.type === "boolean") return { type: "Bool", enum: false };
    if (schema.type === "array" && schema.items) {
      const item = fieldType(schema.items, name + "Item", path + ".items");
      if (item.enum) unsupported(path);
      return { type: `[${item.type}]`, enum: false };
    }
    if (schema.type === "object") {
      if (
        schema.additionalProperties === true &&
        !Object.keys(schema.properties ?? {}).length &&
        !schema.patternProperties
      )
        return { type: "[String: Any]", enum: false };
      if (!Object.keys(schema.properties ?? {}).length && schema.patternProperties) {
        const values = Object.values(schema.patternProperties);
        if (values.length !== 1) unsupported(path);
        const value = fieldType(values[0]!, name + "Value", path + ".patternProperties");
        if (value.enum) unsupported(path);
        return { type: `[String: ${value.type}]`, enum: false };
      }
    }
    return unsupported(path);
  }
  /** `decode: false` for shapes Swift only sends (page results). */
  function structure(name: string, schema: Schema, method?: string, decode = true) {
    checkKeys(schema, name);
    if (schema.type !== "object" || !schema.properties || schema.additionalProperties !== false)
      unsupported(name);
    for (const key of Object.keys(schema.properties)) {
      if (!/^[a-zA-Z][a-zA-Z0-9_]*$/.test(key)) unsupported(name + "." + key);
    }
    const fields = Object.entries(schema.properties)
      .filter(([key]) => key !== "method" || method === undefined)
      .map(([key, value]) => ({
        key,
        name: identifier(key),
        ...fieldType(value, name + title(key), name + "." + key),
        optional: !schema.required?.includes(key),
      }));
    // Only values the type system can prove immutable cross actors.
    const sendable = fields.every((f) => !f.type.includes("Any"));
    const lines = [`public struct ${name}${sendable ? ": Sendable" : ""} {`];
    for (const f of fields) lines.push(`  public var ${f.name}: ${f.type}${f.optional ? "?" : ""}`);
    lines.push(
      `\n  public init(${fields.map((f) => `${f.name}: ${f.type}${f.optional ? "? = nil" : ""}`).join(", ")}) {`,
    );
    for (const f of fields) lines.push(`    self.${f.name} = ${f.name}`);
    lines.push("  }");
    if (decode) {
      lines.push(
        "\n  /// Validate the envelope with Envelope.valid before mapping it.",
        "  public init(json: [String: Any]) throws {",
      );
      for (const f of fields) {
        const access = `json[${quote(f.key)}]`;
        const expression = f.enum
          ? `(${access} as? String).flatMap(${f.type}.init(rawValue:))`
          : f.type === "Any"
            ? access
            : `${access} as? ${f.type}`;
        if (f.optional) {
          lines.push(`    if let value = ${access} {`);
          const converted = f.enum
            ? `(value as? String).flatMap(${f.type}.init(rawValue:))`
            : `value as? ${f.type}`;
          lines.push(
            ...(f.type === "Any"
              ? [`      self.${f.name} = value`]
              : [
                  `      guard let mapped = ${converted} else { throw ContractMappingError.field(${quote(name + "." + f.key)}) }`,
                  `      self.${f.name} = mapped`,
                ]),
            "    } else {",
            `      self.${f.name} = nil`,
            "    }",
          );
        } else {
          lines.push(
            `    guard let ${f.name} = ${expression} else { throw ContractMappingError.field(${quote(name + "." + f.key)}) }`,
            `    self.${f.name} = ${f.name}`,
          );
        }
      }
      lines.push("  }");
    }
    lines.push("\n  public var json: [String: Any] {", "    var result: [String: Any] = [:]");
    if (method !== undefined) lines.push(`    result["method"] = ${quote(method)}`);
    for (const f of fields) {
      if (f.optional)
        lines.push(
          `    if let value = ${f.name} { result[${quote(f.key)}] = value${f.enum ? ".rawValue" : ""} }`,
        );
      else lines.push(`    result[${quote(f.key)}] = ${f.name}${f.enum ? ".rawValue" : ""}`);
    }
    lines.push("    return result", "  }", "}");
    declarations.push(lines.join("\n"));
    return sendable;
  }
  const union = request as Schema;
  if (!union.anyOf || Object.keys(union).some((k) => k !== "anyOf")) unsupported("SocketRequest");
  const variants = union.anyOf.flatMap((schema, index) => {
    const discriminator = schema.properties?.method;
    const methodPath = `SocketRequest.anyOf.${index}.method`;
    if (!discriminator) unsupported(methodPath);
    checkKeys(discriminator, methodPath);
    const methods =
      discriminator?.enum ??
      (typeof discriminator?.const === "string"
        ? [discriminator.const]
        : unsupported(`SocketRequest.anyOf.${index}.method`));
    return methods.map((value) => {
      if (typeof value !== "string" || !/^[a-zA-Z][a-zA-Z0-9._-]*$/.test(value))
        unsupported(methodPath);
      const method = value as string;
      const name = "Socket" + title(method) + "Request";
      const sendable = structure(name, schema, method);
      return { method, name, sendable, epoch: schema.required?.includes("epoch") ?? false };
    });
  });
  if (new Set(variants.map((v) => identifier(v.method))).size !== variants.length)
    unsupported("SocketRequest.method");
  const lines = [`public enum SocketRequest${variants.every((v) => v.sendable) ? ": Sendable" : ""} {`];
  for (const v of variants) lines.push(`  case ${identifier(v.method)}(${v.name})`);
  lines.push("\n  public enum Method: String, CaseIterable, Sendable {");
  for (const v of variants) lines.push(`    case ${identifier(v.method)} = ${quote(v.method)}`);
  lines.push("    public var requiresEpoch: Bool {", "      switch self {");
  for (const v of variants) lines.push(`      case .${identifier(v.method)}: return ${v.epoch}`);
  lines.push("      }", "    }", "  }", "\n  public var method: Method {", "    switch self {");
  for (const v of variants)
    lines.push(`    case .${identifier(v.method)}: return .${identifier(v.method)}`);
  lines.push(
    "    }",
    "  }",
    "  public var requiresEpoch: Bool { method.requiresEpoch }",
    "  public var documentPath: String {",
    "    switch self {",
  );
  for (const v of variants)
    lines.push(`    case .${identifier(v.method)}(let value): return value.documentPath`);
  lines.push("    }", "  }", "  public var epoch: String? {", "    switch self {");
  for (const v of variants)
    lines.push(
      v.epoch
        ? `    case .${identifier(v.method)}(let value): return value.epoch`
        : `    case .${identifier(v.method)}: return nil`,
    );
  lines.push("    }", "  }", "\n  public func with(epoch: String) -> Self {", "    switch self {");
  for (const v of variants)
    lines.push(
      v.epoch
        ? `    case .${identifier(v.method)}(var value): value.epoch = epoch; return .${identifier(v.method)}(value)`
        : `    case .${identifier(v.method)}: return self`,
    );
  lines.push(
    "    }",
    "  }",
    "\n  /// Validate the envelope with Envelope.valid before mapping it.",
    "  public init(json: [String: Any]) throws {",
    '    guard let raw = json["method"] as? String, let method = Method(rawValue: raw) else { throw ContractMappingError.field("SocketRequest.method") }',
    "    switch method {",
  );
  for (const v of variants)
    lines.push(
      `    case .${identifier(v.method)}: self = .${identifier(v.method)}(try ${v.name}(json: json))`,
    );
  lines.push("    }", "  }", "\n  public var json: [String: Any] {", "    switch self {");
  for (const v of variants)
    lines.push(`    case .${identifier(v.method)}(let value): return value.json`);
  lines.push("    }", "  }", "}");
  declarations.push(lines.join("\n"));
  structure("SocketReply", reply as Schema);
  structure("SocketDiscovery", discovery as Schema);
  enumeration("PageMethod", Object.keys(pageResults), "PageMethods");
  // One case per page method; a result with fields carries its generated structure.
  const results = Object.entries(pageResults).map(([method, schema]) => {
    const name = "Page" + title(method) + "Result";
    const empty = !Object.keys((schema as Schema).properties ?? {}).length;
    return {
      method,
      name,
      empty,
      sendable: empty || structure(name, schema as Schema, undefined, false),
    };
  });
  const page = [
    "/// A successful page reply. `json` adds `ok`; failures use `DocumentOwner.pageFailure`.",
    `public enum PageResult${results.every((r) => r.sendable) ? ": Sendable" : ""} {`,
    ...results.map((r) => `  case ${identifier(r.method)}${r.empty ? "" : `(${r.name})`}`),
    "\n  public var json: [String: Any] {",
    "    var result: [String: Any]",
    "    switch self {",
    ...results.map((r) =>
      r.empty
        ? `    case .${identifier(r.method)}: result = [:]`
        : `    case .${identifier(r.method)}(let value): result = value.json`,
    ),
    "    }",
    '    result["ok"] = true',
    "    return result",
    "  }",
    "}",
  ];
  declarations.push(page.join("\n"));
  return (
    "// Generated by bun run schema:generate. Do not edit.\nimport Foundation\n\nprivate enum ContractMappingError: Error { case field(String) }\n\n" +
    declarations.join("\n\n") +
    "\n"
  );
}
