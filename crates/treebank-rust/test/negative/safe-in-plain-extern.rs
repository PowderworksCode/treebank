// RFC 3484 `safe` qualifiers are only legal inside `unsafe extern` blocks;
// rustc hard-rejects them in a plain extern block on every edition.
extern "C" {
    safe fn get_random_u64() -> u64;
}
