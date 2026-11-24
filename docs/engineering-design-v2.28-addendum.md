# Engineering Design v2.28 Addendum — Non-fiction Recovery Verification

This addendum records terminal-level verification of the non-fiction removal path described in the #63 lifecycle contract.

## Verified behavior

- A populated section with a nested subsection and heading-owned prose presents explicit promote, rehome, recoverable subtree-delete, and cancel behavior through the Outline Delete action.
- The selected subtree-delete action removes the heading and its owned prose from active Compile output.
- Reopening the project and restoring the removed root recovers the same child relationship and prose content.
- The integration fixture uses a non-fiction project and does not create scene entities.

## Test stability

The Linux PTY suite serialises its terminal subprocesses. This avoids concurrent terminal startup and key dispatch changing the selection in one PTY while another full-screen application is starting.

The test suite remains an implementation check, not human acceptance. #63 remains open for usability testing and its wider deletion/reference matrix.
