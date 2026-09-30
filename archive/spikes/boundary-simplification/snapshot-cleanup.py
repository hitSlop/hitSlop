"""Remove log bookkeeping made unnecessary by snapshot-only persistence."""
from pathlib import Path
import sys, re
root=Path(__file__).resolve().parents[3]/'generated/boundary-simplification'/sys.argv[1]
p=root/'apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/DocumentOwner.swift'
s=p.read_text()
s=re.sub(r'  /// Stored sizes[^\n]*\n  private var stored:[^\n]*\n', '', s)
s=re.sub(r'    let stored: [^\n]*\n', '', s)
s=re.sub(r'^\s*(?:self\.)?stored = .*\n', '', s, flags=re.M)
s=s.replace('    let meta = try storage.metadata()\n', '')
s=s.replace('return Restored(core: core, stored: (meta.rows, meta.updateBytes, meta.checkpointBytes))', 'return Restored(core: core)')
s=s.replace('    let bytes: Int64\n', '').replace('    let checkpoint: Bool\n', '')
s=s.replace('    let forceCheckpoint = waiters.contains { $0.checkpoint }\n', '')
s=s.replace(' || forceCheckpoint', '').replace('settle(checkpointed: false)', 'settle()').replace('settle(checkpointed: job.checkpoint)', 'settle()')
s=s.replace('checkpoint: true, bytes: Int64(checkpoint.count), ', '')
s=s.replace('private func settle(checkpointed: Bool)', 'private func settle()').replace(', !waiter.checkpoint || checkpointed', '')
s=s.replace('addWaiter(checkpoint: false)', 'addWaiter()').replace('private func addWaiter(checkpoint: Bool, _ resume:', 'private func addWaiter(_ resume:')
s=s.replace('Waiter(target: sequence, checkpoint: checkpoint, ', 'Waiter(target: sequence, ').replace('self.addWaiter(checkpoint: checkpoint)', 'self.addWaiter()')
s=s.replace('private func write(checkpoint: Bool)', 'private func write()').replace('write(checkpoint: false)', 'write()').replace('write(checkpoint: true)', 'write()')
s=s.replace('        sequence = Int(try core.sequence())', '      sequence = Int(try core.sequence())')
p.write_text(s)
