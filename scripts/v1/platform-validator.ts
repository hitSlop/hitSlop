/** Native validation of platform envelopes only. No document-value schema lives in Swift. */
export const platformValidatorSource = String.raw`
import Foundation
import CoreFoundation

public enum PlatformContract {
    public static func valid(_ value: Any, against schema: [String: Any]) -> Bool {
        if let forbidden = schema["not"] as? [String: Any], valid(value, against: forbidden) {
            return false
        }
        if let variants = schema["anyOf"] as? [[String: Any]] {
            return variants.contains { valid(value, against: $0) }
        }
        if let literal = schema["const"] {
            guard String(describing: value) == String(describing: literal) else { return false }
        }
        if let values = schema["enum"] as? [String] {
            guard let text = value as? String, values.contains(text) else { return false }
        }
        switch schema["type"] as? String {
        case "null": return value is NSNull
        case "boolean":
            guard let number = value as? NSNumber else { return false }
            return CFGetTypeID(number) == CFBooleanGetTypeID()
        case "string":
            guard let text = value as? String else { return false }
            if schema["format"] as? String == "uri", URL(string: text, encodingInvalidCharacters: false)?.scheme == nil { return false }
            if let min = schema["minLength"] as? Int, text.utf16.count < min { return false }
            if let max = schema["maxLength"] as? Int, text.utf16.count > max { return false }
            if let pattern = schema["pattern"] as? String, text.range(of: pattern, options: .regularExpression) == nil { return false }
        case "number", "integer":
            guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID() else { return false }
            let n = number.doubleValue
            guard n.isFinite else { return false }
            if schema["type"] as? String == "integer", n.rounded() != n { return false }
            if let min = schema["minimum"] as? Double, n < min { return false }
            if let max = schema["maximum"] as? Double, n > max { return false }
            if let min = schema["exclusiveMinimum"] as? Double, n <= min { return false }
        case "array":
            guard let array = value as? [Any] else { return false }
            if let min = schema["minItems"] as? Int, array.count < min { return false }
            if let max = schema["maxItems"] as? Int, array.count > max { return false }
            if schema["uniqueItems"] as? Bool == true, Set(array.map { String(describing: $0) }).count != array.count { return false }
            if let items = schema["items"] as? [String: Any], !array.allSatisfy({ valid($0, against: items) }) { return false }
        case "object":
            guard let object = value as? [String: Any] else { return false }
            let properties = schema["properties"] as? [String: [String: Any]] ?? [:]
            if let required = schema["required"] as? [String], !required.allSatisfy({ object[$0] != nil }) { return false }
            if schema["additionalProperties"] as? Bool == false, object.keys.contains(where: { properties[$0] == nil }) { return false }
            for (key, value) in object {
                if let child = properties[key], !valid(value, against: child) { return false }
            }
        default: break
        }
        return true
    }
}
`;
