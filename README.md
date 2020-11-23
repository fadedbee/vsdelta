# vsdelta
A command for making and safely applying binary deltas.

## File Format

- 7 bytes of magic: "vsdelta"
- OP_VERSION record
- zero or more OP_XXX records
- OP_END record

The vsdelta format are instructions for a very limited virtual machine (vsapply).

### OP_XXX records

#### OP_VERSION
- 1 byte of OP_HEAD (0x00)
- 3 bytes of version number

If this is not an understood version, vsapply should exit with an error.

#### OP_SHA256_A
- 1 byte of OP_SHA256_A (0xAA)
- 32 bytes of expected sha256

If this hash does not match the hash of file_a, vsapply should exit with an error.

#### OP_SHA256_B
- 1 byte of OP_SHA256_A (0xBB)
- 32 bytes of expected sha256

If this hash does not match the hash of file_b, vsapply should exit with an error.
Note: This opcode only make sense if there are no further OP_SKIP, OP_ADD or OP_HOLEs.

#### OP_LEN_A
- 1 bytes of OP_LEN_A (0x77)
- 8 bytes of expected file_a length

#### OP_LEN_B
- 1 bytes of OP_LEN_A (0x88)
- 8 bytes of expected file_a length

#### OP_SKIP file_b is same as file_a
- 1 byte of OP_SKIP (0x55)
- 8 bytes of count

in-place: The file_a pointer should be advanced by "count" bytes.
external: "count" bytes should be copied from file_a to file_b (the output).

#### OP_DIFF file_b is different from file_a
- 1 byte of OP_DIFF (0xDD)
- 8 bytes of count
- count bytes of data

in-place: "count" bytes should be copied from delta to file_a.
external: "count" bytes should be copied from delta to file_b (the output).

#### OP_HOLE
- 1 byte of OP_HOLE (0x44)
- 8 bytes of count

in-place: The file_a pointer should be advanced by "count" bytes, if those bytes are all zero, otherwise non-zero bytes should be zeroed.
external: The file_b pointer should be advanced by "count" bytes.

#### OP_END 
- 1 byte of OP_END (0xEE)



Open question: https://stackoverflow.com/questions/64951749/meta-information-at-the-start-or-the-end-of-a-delta-file-format