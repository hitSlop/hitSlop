import Foundation
import PopCoreBinding

// Presentation and routing only. Rust owns authority decoding, validation and encoding.
extension CatalogAuthority {
  var isSnapshot: Bool {
    switch self {
    case .snapshot, .checkpoint: return true
    default: return false
    }
  }
  var transitionID: String? {
    switch self {
    case .snapshot(let receipt): return receipt?.transition
    case .checkpoint(let proof): return proof.transition
    case .preparing(let transition, _, _, _), .relay(let transition, _, _), .handoff(let transition, _, _):
      return transition
    }
  }
  var checkpointProof: CatalogCheckpointProof? {
    if case .checkpoint(let proof) = self { return proof }
    return nil
  }
  var roomID: String? {
    switch self {
    case .snapshot(let receipt): return receipt?.room
    case .checkpoint: return nil
    case .preparing(_, let room, _, _), .relay(_, let room, _), .handoff(_, let room, _): return room
    }
  }
  var relayIdentity: (room: String, seedHash: String)? {
    guard let binding = try? libraryRelayBinding(authority: self) else { return nil }
    return (binding.room, binding.seedHash)
  }
}
