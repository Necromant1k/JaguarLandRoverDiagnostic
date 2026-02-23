#!/usr/bin/env python3
"""
EXML decryptor for JLR SDD files
Ported from https://github.com/smartgauges/exml
"""

import sys
from Crypto.Cipher import DES3

# Hardcoded 3DES key from the original C++ code
KEY = bytes([
    0x59, 0x6d, 0x5a, 0x77, 0x5a, 0x6c, 0x51, 0x72,
    0x51, 0x33, 0x56, 0x34, 0x64, 0x56, 0x6c, 0x74,
    0x4e, 0x54, 0x41, 0x72, 0x57, 0x45, 0x39, 0x73
])

def decrypt_file(input_path, output_path):
    """Decrypt an EXML file using 3DES ECB mode"""
    cipher = DES3.new(KEY, DES3.MODE_ECB)

    with open(input_path, 'rb') as f_in:
        encrypted_data = f_in.read()

    # Decrypt
    decrypted_data = cipher.decrypt(encrypted_data)

    # Remove PKCS7 padding from the last block
    padding_len = decrypted_data[-1]
    if padding_len < 8:  # Valid padding should be 1-8
        decrypted_data = decrypted_data[:-padding_len]

    with open(output_path, 'wb') as f_out:
        f_out.write(decrypted_data)

    print(f"Decrypted: {input_path} -> {output_path}")

def encrypt_file(input_path, output_path):
    """Encrypt a file to EXML format using 3DES ECB mode"""
    cipher = DES3.new(KEY, DES3.MODE_ECB)

    with open(input_path, 'rb') as f_in:
        plaintext = f_in.read()

    # Add PKCS7 padding
    padding_len = 8 - (len(plaintext) % 8)
    if padding_len == 0:
        padding_len = 8
    plaintext += bytes([padding_len] * padding_len)

    # Encrypt
    encrypted_data = cipher.encrypt(plaintext)

    with open(output_path, 'wb') as f_out:
        f_out.write(encrypted_data)

    print(f"Encrypted: {input_path} -> {output_path}")

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} [-decrypt|-encrypt] <input_file> [output_file]")
        print(f"  Default mode: decrypt")
        print(f"  If output_file not specified, adds suffix to input filename")
        sys.exit(1)

    mode = 'decrypt'
    input_file = None
    output_file = None

    if sys.argv[1] == '-decrypt':
        mode = 'decrypt'
        input_file = sys.argv[2] if len(sys.argv) > 2 else None
        output_file = sys.argv[3] if len(sys.argv) > 3 else None
    elif sys.argv[1] == '-encrypt':
        mode = 'encrypt'
        input_file = sys.argv[2] if len(sys.argv) > 2 else None
        output_file = sys.argv[3] if len(sys.argv) > 3 else None
    else:
        # No mode specified, assume decrypt
        input_file = sys.argv[1]
        output_file = sys.argv[2] if len(sys.argv) > 2 else None

    if not input_file:
        print("Error: input file required")
        sys.exit(1)

    # Generate output filename if not specified
    if not output_file:
        if mode == 'decrypt':
            output_file = input_file + '-decrypted'
        else:
            output_file = input_file + '-encrypted'

    # Perform encryption/decryption
    try:
        if mode == 'decrypt':
            decrypt_file(input_file, output_file)
        else:
            encrypt_file(input_file, output_file)
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

if __name__ == '__main__':
    main()
