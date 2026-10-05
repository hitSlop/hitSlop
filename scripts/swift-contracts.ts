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
  pageRequest: TSchema,
  pageFailure: TSchema,
  pageResults: Record<string, TSchema>,
  hostRequest: TSchema,
  hostCaptureResult: TSchema,
) {
  const declarations: string[] = [];
  /** Each emitted enumeration's cases, by type name. */
  const declared = new Map<string, string>();
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
  /** A titled enumeration is one type wherever it appears. */
  function enumeration(name: string, values: unknown[], path: string) {
    if (
      !values.length ||
      values.some((v) => typeof v !== "string" || !/^[a-zA-Z][a-zA-Z0-9._-]*$/.test(v))
    )
      unsupported(path);
    if (new Set(values.map((v) => identifier(v as string))).size !== values.length)
      unsupported(path);
    const cases = values.join("|");
    const existing = declared.get(name);
    if (existing !== undefined) {
      if (existing !== cases) throw new Error(`Swift enumeration ${name} has two definitions (${path})`);
      return name;
    }
    declared.set(name, cases);
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
    if (schema.enum) return { type: enumeration((schema.title as string | undefined) ?? name, schema.enum, path), enum: true };
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
    // A boolean constant (`ok: false`) is written by `json`, never stored.
    const constants = Object.entries(schema.properties).filter(([, value]) => typeof value.const === "boolean");
    if (decode && constants.length) unsupported(name);
    const fields = Object.entries(schema.properties)
      .filter(([key]) => key !== "method" || method === undefined)
      .filter(([, value]) => typeof value.const !== "boolean")
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
    for (const [key, value] of constants) lines.push(`    result[${quote(key)}] = ${value.const}`);
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
  /** A union of requests discriminated by `method`. Socket requests are routed: each names
   * its document, and mutations carry the owner's epoch. */
  function requests(name: string, schema: TSchema) {
    const union = schema as Schema;
    if (!union.anyOf || Object.keys(union).some((k) => k !== "anyOf")) unsupported(name);
    const prefix = name.replace(/Request$/, "");
    const variants = union.anyOf.flatMap((schema, index) => {
      const discriminator = schema.properties?.method;
      const methodPath = `${name}.anyOf.${index}.method`;
      if (!discriminator) unsupported(methodPath);
      checkKeys(discriminator, methodPath);
      const methods =
        discriminator?.enum ??
        (typeof discriminator?.const === "string" ? [discriminator.const] : unsupported(methodPath));
      return methods.map((value) => {
        if (typeof value !== "string" || !/^[a-zA-Z][a-zA-Z0-9._-]*$/.test(value)) unsupported(methodPath);
        const method = value as string;
        const type = prefix + title(method) + "Request";
        const sendable = structure(type, schema, method);
        return { method, name: type, sendable, epoch: schema.required?.includes("epoch") ?? false };
      });
    });
    if (new Set(variants.map((v) => identifier(v.method))).size !== variants.length) unsupported(`${name}.method`);
    const routed = union.anyOf.every((schema) => schema.required?.includes("documentPath"));
    const lines = [`public enum ${name}${variants.every((v) => v.sendable) ? ": Sendable" : ""} {`];
    for (const v of variants) lines.push(`  case ${identifier(v.method)}(${v.name})`);
    lines.push("\n  public enum Method: String, CaseIterable, Sendable {");
    for (const v of variants) lines.push(`    case ${identifier(v.method)} = ${quote(v.method)}`);
    if (routed) {
      lines.push("    public var requiresEpoch: Bool {", "      switch self {");
      for (const v of variants) lines.push(`      case .${identifier(v.method)}: return ${v.epoch}`);
      lines.push("      }", "    }");
    }
    lines.push("  }", "\n  public var method: Method {", "    switch self {");
    for (const v of variants) lines.push(`    case .${identifier(v.method)}: return .${identifier(v.method)}`);
    lines.push("    }", "  }");
    if (routed) {
      lines.push("  public var requiresEpoch: Bool { method.requiresEpoch }", "  public var documentPath: String {", "    switch self {");
      for (const v of variants) lines.push(`    case .${identifier(v.method)}(let value): return value.documentPath`);
      lines.push("    }", "  }", "  public var epoch: String? {", "    switch self {");
      for (const v of variants)
        lines.push(v.epoch ? `    case .${identifier(v.method)}(let value): return value.epoch` : `    case .${identifier(v.method)}: return nil`);
      lines.push("    }", "  }", "\n  public func with(epoch: String) -> Self {", "    switch self {");
      for (const v of variants)
        lines.push(
          v.epoch
            ? `    case .${identifier(v.method)}(var value): value.epoch = epoch; return .${identifier(v.method)}(value)`
            : `    case .${identifier(v.method)}: return self`,
        );
      lines.push("    }", "  }");
    }
    lines.push(
      "\n  /// Validate the envelope with Envelope.valid before mapping it.",
      "  public init(json: [String: Any]) throws {",
      `    guard let raw = json["method"] as? String, let method = Method(rawValue: raw) else { throw ContractMappingError.field("${name}.method") }`,
      "    switch method {",
    );
    for (const v of variants) lines.push(`    case .${identifier(v.method)}: self = .${identifier(v.method)}(try ${v.name}(json: json))`);
    lines.push("    }", "  }", "\n  public var json: [String: Any] {", "    switch self {");
    for (const v of variants) lines.push(`    case .${identifier(v.method)}(let value): return value.json`);
    lines.push("    }", "  }", "}");
    declarations.push(lines.join("\n"));
  }
  /** Complete socket successes, and one classified failure. Core-owned state remains
   * JSON text until the encoder splices it into the envelope. */
  function socketReplies(schema: TSchema) {
    const union = schema as Schema;
    if (!union.anyOf) unsupported("SocketReply");
    const failure = union.anyOf.find((member) => member.properties?.ok?.const === false);
    if (!failure) unsupported("SocketReply.failure");
    structure("SocketFailure", failure, undefined, false);
    const variants = union.anyOf.filter((member) => member !== failure).map((member) => {
      checkKeys(member, "SocketReply");
      const method = member.properties?.method?.const;
      if (typeof method !== "string" || member.properties?.ok?.const !== true) unsupported("SocketReply.method");
      const fields = Object.entries(member.properties!).filter(([key]) => key !== "ok" && key !== "method").map(([key, value]) => ({
        key,
        name: identifier(key === "state" ? "stateJSON" : key),
        type: key === "state" ? "String" : fieldType(value, "Socket" + title(method) + title(key), "SocketReply." + method + "." + key).type,
        optional: !member.required?.includes(key),
      })).sort((a, b) => Number(a.optional) - Number(b.optional));
      return { method, fields };
    });
    const lines = ["/// A complete reply. A success cannot be constructed without its method's result.", "public enum SocketReply: Sendable {"];
    for (const variant of variants) {
      lines.push(`  case ${identifier(variant.method)}(${variant.fields.map((f) => `${f.name}: ${f.type}${f.optional ? "?" : ""}`).join(", ")})`);
    }
    lines.push("  case failure(SocketFailure)", "", "  private var header: [String: Any] {", "    switch self {");
    for (const variant of variants) {
      const pattern = variant.fields.map((f) => f.key === "state" ? "_" : `let ${f.name}`).join(", ");
      lines.push(`    case .${identifier(variant.method)}(${pattern}):`);
      const header = [`"ok": true`, `"method": ${quote(variant.method)}`, ...variant.fields.filter((f) => f.key !== "state" && !f.optional).map((f) => `${quote(f.key)}: ${f.name}`)];
      const optional = variant.fields.filter((f) => f.optional);
      if (optional.length) {
        lines.push(`      var result: [String: Any] = [${header.join(", ")}]`);
        for (const field of optional) lines.push(`      if let ${field.name} { result[${quote(field.key)}] = ${field.name} }`);
        lines.push("      return result");
      } else lines.push(`      return [${header.join(", ")}]`);
    }
    lines.push("    case .failure(let failure): return failure.json", "    }", "  }", "", "  private var stateJSON: String? {", "    switch self {");
    for (const variant of variants.filter((v) => v.fields.some((f) => f.key === "state"))) {
      lines.push(`    case .${identifier(variant.method)}(${variant.fields.map((f) => f.key === "state" ? "let state" : "_").join(", ")}): return state`);
    }
    lines.push("    default: return nil", "    }", "  }", "", "  /// Encodes routing fields and splices the core's state without interpreting it.", "  public func encoded() -> Data {", "    guard var bytes = try? JSONSerialization.data(withJSONObject: header, options: .withoutEscapingSlashes) else {",
      '      return Data(#"{"ok":false,"code":"unknown_outcome","error":"Invalid response. Outcome unknown; run slop get before another edit."}"#.utf8)',
      "    }", "    if let stateJSON {", "      bytes.removeLast()", '      bytes.append(contentsOf: #",\"state\":"#.utf8)', "      bytes.append(contentsOf: stateJSON.utf8)", '      bytes.append(UInt8(ascii: "}"))', "    }", "    return bytes", "  }", "}");
    declarations.push(lines.join("\n"));
    // Helpers only inspect this header; the state projection is skipped by JSONDecoder.
    declarations.push(`/// Routing and outcome metadata read beside the original reply bytes; never a result.
public struct SocketReplyHeader: Decodable, Sendable {
  public let ok: Bool
  public let method: SocketRequest.Method?
  public let epoch: String?
  public let coreBuildId: String?
  public let error: String?
  public let code: OutcomeCode?
  public let reason: CoreErrorCode?
  public let opIndex: Int?
  private enum CodingKeys: String, CodingKey { case ok, method, epoch, coreBuildId, error, code, reason, opIndex }
  public init(from decoder: Decoder) throws {
    let fields = try decoder.container(keyedBy: CodingKeys.self)
    ok = try fields.decode(Bool.self, forKey: .ok)
    epoch = try fields.decodeIfPresent(String.self, forKey: .epoch)
    coreBuildId = try fields.decodeIfPresent(String.self, forKey: .coreBuildId)
    error = try fields.decodeIfPresent(String.self, forKey: .error)
    opIndex = try fields.decodeIfPresent(Int.self, forKey: .opIndex)
    method = try fields.decodeIfPresent(String.self, forKey: .method).flatMap(SocketRequest.Method.init(rawValue:))
    code = try fields.decodeIfPresent(String.self, forKey: .code).flatMap(OutcomeCode.init(rawValue:))
    reason = try fields.decodeIfPresent(String.self, forKey: .reason).flatMap(CoreErrorCode.init(rawValue:))
    guard ok ? method != nil : (code != nil && error != nil) else {
      throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "Invalid socket reply header"))
    }
  }
}`);
  }
  requests("SocketRequest", request);
  requests("PageRequest", pageRequest);
  requests("HostRequest", hostRequest);
  structure("HostCaptureResult", hostCaptureResult as Schema);
  structure("PageFailure", pageFailure as Schema, undefined, false);
  socketReplies(reply);
  structure("SocketDiscovery", discovery as Schema);
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
    "/// A successful page reply. `json` adds `ok`; failures are `PageFailure`.",
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
