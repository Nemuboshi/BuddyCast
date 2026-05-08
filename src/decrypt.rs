/// Exact byte remapping table used by the original prototype.
///
/// Each encrypted byte is used as an index into this lookup table and the
/// returned value becomes the decrypted output byte.
pub const DECRYPTION_MAP: [u8; 256] = [
    28, 42, 12, 24, 36, 125, 3, 46, 123, 15, 7, 39, 40, 13, 8, 93, 44, 95, 33, 34, 126, 127, 61, 0,
    16, 23, 4, 14, 92, 6, 1, 19, 32, 124, 20, 35, 27, 37, 38, 63, 9, 59, 62, 94, 29, 10, 31, 21,
    79, 99, 75, 65, 87, 120, 82, 118, 53, 57, 30, 18, 43, 26, 11, 5, 25, 97, 49, 69, 48, 114, 84,
    116, 85, 55, 121, 122, 108, 52, 51, 73, 76, 113, 70, 83, 81, 105, 119, 78, 88, 89, 54, 45, 22,
    41, 96, 64, 60, 110, 80, 106, 109, 98, 115, 66, 77, 72, 56, 100, 74, 107, 90, 71, 102, 68, 101,
    50, 117, 111, 103, 86, 112, 104, 67, 17, 58, 91, 2, 47, 138, 189, 232, 212, 135, 173, 151, 145,
    201, 194, 253, 163, 245, 235, 140, 187, 251, 236, 174, 131, 199, 221, 195, 165, 144, 186, 172,
    141, 168, 148, 230, 209, 247, 192, 190, 249, 207, 181, 244, 231, 254, 178, 255, 130, 166, 154,
    197, 205, 156, 237, 241, 223, 170, 164, 220, 224, 136, 134, 188, 246, 157, 213, 183, 177, 143,
    193, 252, 137, 155, 180, 196, 182, 128, 146, 203, 238, 132, 162, 210, 200, 211, 158, 161, 243,
    160, 152, 149, 159, 185, 202, 248, 150, 242, 239, 222, 240, 153, 225, 167, 227, 228, 184, 229,
    176, 233, 218, 198, 175, 206, 171, 216, 204, 139, 219, 191, 129, 234, 226, 250, 133, 179, 142,
    215, 217, 208, 214, 147, 169,
];

const ZIP_SIGNATURES: [&[u8; 4]; 3] = [b"PK\x03\x04", b"PK\x05\x06", b"PK\x07\x08"];

/// Decrypt a full byte slice by applying the lookup table to every byte.
pub fn decrypt_bytes(data: &[u8]) -> Vec<u8> {
    data.iter()
        .map(|byte| DECRYPTION_MAP[*byte as usize])
        .collect()
}

/// Check whether a byte slice starts with a known zip file signature.
pub fn is_zip_bytes(data: &[u8]) -> bool {
    ZIP_SIGNATURES
        .iter()
        .any(|signature| data.starts_with(signature.as_slice()))
}
