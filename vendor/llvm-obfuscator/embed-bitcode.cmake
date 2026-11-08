file(READ "${INPUT}" bitcode HEX)
string(REGEX REPLACE "(..)" "0x\\1," bytes "${bitcode}")
file(WRITE "${OUTPUT}" "static const unsigned char decode_c_bc[] = {${bytes}};\nstatic const unsigned int decode_c_bc_len = sizeof(decode_c_bc);\n")
