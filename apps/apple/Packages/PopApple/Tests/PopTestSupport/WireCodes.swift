public enum CoreErrorCode: String, Sendable {
  case typeMismatch = "type_mismatch"
  case outOfRange = "out_of_range"
  case pathNotFound = "path_not_found"
  case invalidKey = "invalid_key"
  case exists
  case duplicateId = "duplicate_id"
  case invalidRequest = "invalid_request"
  case invalidId = "invalid_id"
  case invalidPath = "invalid_path"
  case invalidSchema = "invalid_schema"
  case tooLarge = "too_large"
  case staleBase = "stale_base"
  case invalidBytes = "invalid_bytes"
  case missingDependencies = "missing_dependencies"
  case engineError = "engine_error"
  case invalidShape = "invalid_shape"
  case requiresUpdate = "requires_update"
  case isTemplate = "is_template"
  case refused
}
