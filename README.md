# vsdelta
A command for making and safely applying binary deltas.

## File Format

- 7 bytes of magic: "vsdelta"
- 84 bytes of metadata
  - 1 byte of OP_HEAD (0x00)
  - 3 bytes of version number
  - 8 bytes of file_a length
  - 32 bytes of sha256(file_a)
  - 8 bytes of file_b length
  - 32 bytes of sha256(file_b)
- zero or more OP_XXX records
- 1 byte of OP_END

Open question: https://stackoverflow.com/questions/64951749/meta-information-at-the-start-or-the-end-of-a-delta-file-format