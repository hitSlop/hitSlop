"""Adapt observable host behavior to conservative acknowledgement handling."""
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[3] / 'generated/boundary-simplification' / sys.argv[1]
p = root / 'apps/apple/Packages/HitSlopApple/Tests/HitSlopHostTests/OwnerEngineTests.swift'
s = p.read_text()
s = s.replace('guard phase == "append:committed" else { return }', 'guard phase.hasSuffix(":committed") else { return }')
s = s.replace('''    // The socket follows the owner, not the page: the lost acknowledgement is settled by
    // the stored attempt token, so the CLI learns the edit is durable.
    _ = try await DocumentCommand.run(
      method: "apply", url: root, operation: setTitle("Committed before renderer death"))''', '''    // The socket follows the owner. A lost acknowledgement reports failure until
    // a later retry confirms the save; the accepted edit and ownership remain.
    await #expect(throws: (any Error).self) {
      _ = try await DocumentCommand.run(
        method: "apply", url: root, operation: setTitle("Committed before renderer death"))
    }''')
s = s.replace('lostAppendReplyIsRecoveredByGetWithoutReplayingIntent', 'lostAcknowledgementIsRetriedWithoutReplayingIncrement')
s = s.replace('''    _ = try await DocumentCommand.run(
      method: "apply", url: root, operation: setTitle("Lost reply"))''', '''    await #expect(throws: (any Error).self) {
      _ = try await DocumentCommand.run(method: "apply", url: root,
        operation: Data(#"{"type":"increment","path":["hits"],"by":1}"#.utf8))
    }''')
s = s.replace('''    #expect(String(decoding: bytes, as: UTF8.self).contains("Lost reply"))''', '''    #expect((try JSONSerialization.jsonObject(with: bytes) as? [String: Any])?["hits"] as? Int == 1)''')
p.write_text(s)
