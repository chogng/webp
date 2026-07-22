# Invalidated identity helper invocation

The first full-identity attempt reused the P13 helper, which requires all
Default, Compact, and LowLatency streams to be byte-identical across archives.
P14 explicitly permits standard fast-profile VP8L bytes to change, so that
helper stopped on the first Compact stream. No decoder failed. The partial
rows and metadata are retained here.

The P14-specific `verify_identity.py` instead requires Default full-byte
identity and complete RGBA equality for every profile. Its valid results are
in `raw/identity-306-final/`.
