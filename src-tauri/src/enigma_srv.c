/*
 * NeuroVeil Enigma v1 — Lightweight Polymorphic UDP Anti-DPI Server Daemon
 * Zero external library dependencies (pure C99 + RFC 8439 ChaCha20-Poly1305 + HMAC-SHA256).
 *
 * Build on Linux VPS:
 *   gcc -O3 -std=c99 -Wall -Wextra -o /usr/local/bin/neuroveil-enigma-srv neuroveil-enigma-srv.c
 *
 * Usage:
 *   neuroveil-enigma-srv <listen_port> <wg_upstream_port> <keybook.nvkey>
 */

#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>

#ifndef ENIGMA_SELF_TEST
#include <unistd.h>
#include <fcntl.h>
#include <signal.h>
#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/epoll.h>
#endif

#define ENIGMA_KEYBOOK_LEN   16384
#define ENIGMA_MASTER_OFF    16
#define ENIGMA_HINT_OFF      48
#define ENIGMA_ROTORS_OFF    80
#define ENIGMA_CORPUS_OFF    8272
#define ENIGMA_CORPUS_LEN    8112
#define ENIGMA_TIME_SLOT     30
#define ENIGMA_HDR_LEN       24
#define ENIGMA_TAG_LEN       16
#define MAX_PKT_BUF          4096
#define MAX_SESSIONS         128
#define REPLAY_CAP           1024

static inline uint32_t load32_le(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}
static inline void store32_le(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)(v & 0xff);
    p[1] = (uint8_t)((v >> 8) & 0xff);
    p[2] = (uint8_t)((v >> 16) & 0xff);
    p[3] = (uint8_t)((v >> 24) & 0xff);
}
static inline void store64_le(uint8_t *p, uint64_t v) {
    for (int i = 0; i < 8; i++) p[i] = (uint8_t)((v >> (8 * i)) & 0xff);
}
static inline uint32_t load32_be(const uint8_t *p) {
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | (uint32_t)p[3];
}
static inline void store32_be(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)((v >> 24) & 0xff);
    p[1] = (uint8_t)((v >> 16) & 0xff);
    p[2] = (uint8_t)((v >> 8) & 0xff);
    p[3] = (uint8_t)(v & 0xff);
}
static inline uint16_t load16_be(const uint8_t *p) {
    return (uint16_t)(((uint16_t)p[0] << 8) | (uint16_t)p[1]);
}
static inline void store16_be(uint8_t *p, uint16_t v) {
    p[0] = (uint8_t)((v >> 8) & 0xff);
    p[1] = (uint8_t)(v & 0xff);
}
static inline void store64_be(uint8_t *p, uint64_t v) {
    for (int i = 0; i < 8; i++) p[7 - i] = (uint8_t)((v >> (8 * i)) & 0xff);
}
static inline uint32_t rotl32(uint32_t x, int n) {
    return (x << n) | (x >> (32 - n));
}
static inline uint32_t rotr32(uint32_t x, int n) {
    return (x >> n) | (x << (32 - n));
}

/* ==================== SHA-256 & HMAC-SHA256 ==================== */
typedef struct {
    uint32_t state[8];
    uint64_t bitlen;
    uint8_t data[64];
    size_t datalen;
} sha256_ctx;

static const uint32_t K256[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

static void sha256_transform(sha256_ctx *ctx, const uint8_t data[64]) {
    uint32_t m[64], a, b, c, d, e, f, g, h;
    for (int i = 0; i < 16; i++) m[i] = load32_be(data + i * 4);
    for (int i = 16; i < 64; i++) {
        uint32_t s0 = rotr32(m[i - 15], 7) ^ rotr32(m[i - 15], 18) ^ (m[i - 15] >> 3);
        uint32_t s1 = rotr32(m[i - 2], 17) ^ rotr32(m[i - 2], 19) ^ (m[i - 2] >> 10);
        m[i] = m[i - 16] + s0 + m[i - 7] + s1;
    }
    a = ctx->state[0]; b = ctx->state[1]; c = ctx->state[2]; d = ctx->state[3];
    e = ctx->state[4]; f = ctx->state[5]; g = ctx->state[6]; h = ctx->state[7];
    for (int i = 0; i < 64; i++) {
        uint32_t S1 = rotr32(e, 6) ^ rotr32(e, 11) ^ rotr32(e, 25);
        uint32_t ch = (e & f) ^ ((~e) & g);
        uint32_t t1 = h + S1 + ch + K256[i] + m[i];
        uint32_t S0 = rotr32(a, 2) ^ rotr32(a, 13) ^ rotr32(a, 22);
        uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
        uint32_t t2 = S0 + maj;
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }
    ctx->state[0] += a; ctx->state[1] += b; ctx->state[2] += c; ctx->state[3] += d;
    ctx->state[4] += e; ctx->state[5] += f; ctx->state[6] += g; ctx->state[7] += h;
}

static void sha256_init(sha256_ctx *ctx) {
    ctx->datalen = 0;
    ctx->bitlen = 0;
    ctx->state[0] = 0x6a09e667; ctx->state[1] = 0xbb67ae85;
    ctx->state[2] = 0x3c6ef372; ctx->state[3] = 0xa54ff53a;
    ctx->state[4] = 0x510e527f; ctx->state[5] = 0x9b05688c;
    ctx->state[6] = 0x1f83d9ab; ctx->state[7] = 0x5be0cd19;
}

static void sha256_update(sha256_ctx *ctx, const uint8_t *data, size_t len) {
    for (size_t i = 0; i < len; i++) {
        ctx->data[ctx->datalen++] = data[i];
        if (ctx->datalen == 64) {
            sha256_transform(ctx, ctx->data);
            ctx->bitlen += 512;
            ctx->datalen = 0;
        }
    }
}

static void sha256_final(sha256_ctx *ctx, uint8_t hash[32]) {
    size_t i = ctx->datalen;
    if (ctx->datalen < 56) {
        ctx->data[i++] = 0x80;
        while (i < 56) ctx->data[i++] = 0x00;
    } else {
        ctx->data[i++] = 0x80;
        while (i < 64) ctx->data[i++] = 0x00;
        sha256_transform(ctx, ctx->data);
        memset(ctx->data, 0, 56);
    }
    ctx->bitlen += ctx->datalen * 8;
    store64_be(ctx->data + 56, ctx->bitlen);
    sha256_transform(ctx, ctx->data);
    for (int j = 0; j < 8; j++) store32_be(hash + j * 4, ctx->state[j]);
}

static void sha256_once(const uint8_t *data, size_t len, uint8_t out[32]) {
    sha256_ctx ctx;
    sha256_init(&ctx);
    sha256_update(&ctx, data, len);
    sha256_final(&ctx, out);
}

static void hmac_sha256(const uint8_t *key, size_t key_len,
                        const uint8_t *msg, size_t msg_len,
                        uint8_t out[32]) {
    uint8_t k_pad[64];
    uint8_t inner_hash[32];
    sha256_ctx ctx;

    memset(k_pad, 0, sizeof(k_pad));
    if (key_len > 64) {
        sha256_once(key, key_len, k_pad);
    } else {
        memcpy(k_pad, key, key_len);
    }

    uint8_t ipad[64], opad[64];
    for (int i = 0; i < 64; i++) {
        ipad[i] = k_pad[i] ^ 0x36;
        opad[i] = k_pad[i] ^ 0x5c;
    }
    sha256_init(&ctx);
    sha256_update(&ctx, ipad, 64);
    sha256_update(&ctx, msg, msg_len);
    sha256_final(&ctx, inner_hash);

    sha256_init(&ctx);
    sha256_update(&ctx, opad, 64);
    sha256_update(&ctx, inner_hash, 32);
    sha256_final(&ctx, out);
}

/* ==================== RFC 8439 ChaCha20 + Poly1305 ==================== */
#define QR(a, b, c, d) do { \
    a += b; d ^= a; d = rotl32(d, 16); \
    c += d; b ^= c; b = rotl32(b, 12); \
    a += b; d ^= a; d = rotl32(d, 8);  \
    c += d; b ^= c; b = rotl32(b, 7);  \
} while (0)

static void chacha20_block(const uint8_t key[32], uint32_t counter, const uint8_t nonce[12], uint8_t out[64]) {
    uint32_t s[16], x[16];
    s[0] = 0x61707865; s[1] = 0x3320646e; s[2] = 0x79622d32; s[3] = 0x6b206574;
    for (int i = 0; i < 8; i++) s[4 + i] = load32_le(key + i * 4);
    s[12] = counter;
    s[13] = load32_le(nonce);
    s[14] = load32_le(nonce + 4);
    s[15] = load32_le(nonce + 8);
    memcpy(x, s, sizeof(x));
    for (int i = 0; i < 10; i++) {
        QR(x[0], x[4], x[8],  x[12]);
        QR(x[1], x[5], x[9],  x[13]);
        QR(x[2], x[6], x[10], x[14]);
        QR(x[3], x[7], x[11], x[15]);
        QR(x[0], x[5], x[10], x[15]);
        QR(x[1], x[6], x[11], x[12]);
        QR(x[2], x[7], x[8],  x[13]);
        QR(x[3], x[4], x[9],  x[14]);
    }
    for (int i = 0; i < 16; i++) store32_le(out + i * 4, x[i] + s[i]);
}

static void chacha20_xor(const uint8_t key[32], uint32_t counter, const uint8_t nonce[12],
                         const uint8_t *in, uint8_t *out, size_t len) {
    uint8_t blk[64];
    size_t off = 0;
    while (off < len) {
        chacha20_block(key, counter++, nonce, blk);
        size_t take = len - off;
        if (take > 64) take = 64;
        for (size_t i = 0; i < take; i++) out[off + i] = in[off + i] ^ blk[i];
        off += take;
    }
}

typedef struct {
    uint32_t r[5];
    uint32_t h[5];
    uint32_t pad[4];
    uint8_t buf[16];
    size_t leftover;
} poly1305_ctx;

static void poly1305_init(poly1305_ctx *st, const uint8_t key[32]) {
    st->r[0] = (load32_le(key + 0)) & 0x3ffffff;
    st->r[1] = (load32_le(key + 3) >> 2) & 0x3ffff03;
    st->r[2] = (load32_le(key + 6) >> 4) & 0x3ffc0ff;
    st->r[3] = (load32_le(key + 9) >> 6) & 0x3f03fff;
    st->r[4] = (load32_le(key + 12) >> 8) & 0x00fffff;
    for (int i = 0; i < 5; i++) st->h[i] = 0;
    for (int i = 0; i < 4; i++) st->pad[i] = load32_le(key + 16 + i * 4);
    st->leftover = 0;
}

static void poly1305_blocks(poly1305_ctx *st, const uint8_t *m, size_t bytes, uint32_t hibit) {
    uint32_t r0 = st->r[0], r1 = st->r[1], r2 = st->r[2], r3 = st->r[3], r4 = st->r[4];
    uint32_t s1 = r1 * 5, s2 = r2 * 5, s3 = r3 * 5, s4 = r4 * 5;
    uint32_t h0 = st->h[0], h1 = st->h[1], h2 = st->h[2], h3 = st->h[3], h4 = st->h[4];
    while (bytes >= 16) {
        h0 += (load32_le(m + 0)) & 0x3ffffff;
        h1 += (load32_le(m + 3) >> 2) & 0x3ffffff;
        h2 += (load32_le(m + 6) >> 4) & 0x3ffffff;
        h3 += (load32_le(m + 9) >> 6) & 0x3ffffff;
        h4 += (load32_le(m + 12) >> 8) | hibit;

        uint64_t d0 = (uint64_t)h0 * r0 + (uint64_t)h1 * s4 + (uint64_t)h2 * s3 + (uint64_t)h3 * s2 + (uint64_t)h4 * s1;
        uint64_t d1 = (uint64_t)h0 * r1 + (uint64_t)h1 * r0 + (uint64_t)h2 * s4 + (uint64_t)h3 * s3 + (uint64_t)h4 * s2;
        uint64_t d2 = (uint64_t)h0 * r2 + (uint64_t)h1 * r1 + (uint64_t)h2 * r0 + (uint64_t)h3 * s4 + (uint64_t)h4 * s3;
        uint64_t d3 = (uint64_t)h0 * r3 + (uint64_t)h1 * r2 + (uint64_t)h2 * r1 + (uint64_t)h3 * r0 + (uint64_t)h4 * s4;
        uint64_t d4 = (uint64_t)h0 * r4 + (uint64_t)h1 * r3 + (uint64_t)h2 * r2 + (uint64_t)h3 * r1 + (uint64_t)h4 * r0;

        uint32_t c;
        c = (uint32_t)(d0 >> 26); h0 = (uint32_t)d0 & 0x3ffffff; d1 += c;
        c = (uint32_t)(d1 >> 26); h1 = (uint32_t)d1 & 0x3ffffff; d2 += c;
        c = (uint32_t)(d2 >> 26); h2 = (uint32_t)d2 & 0x3ffffff; d3 += c;
        c = (uint32_t)(d3 >> 26); h3 = (uint32_t)d3 & 0x3ffffff; d4 += c;
        c = (uint32_t)(d4 >> 26); h4 = (uint32_t)d4 & 0x3ffffff; h0 += c * 5;
        c = (h0 >> 26);           h0 &= 0x3ffffff;                h1 += c;

        m += 16;
        bytes -= 16;
    }
    st->h[0] = h0; st->h[1] = h1; st->h[2] = h2; st->h[3] = h3; st->h[4] = h4;
}

static void poly1305_update(poly1305_ctx *st, const uint8_t *m, size_t bytes) {
    if (st->leftover) {
        size_t want = 16 - st->leftover;
        if (want > bytes) want = bytes;
        memcpy(st->buf + st->leftover, m, want);
        bytes -= want;
        m += want;
        st->leftover += want;
        if (st->leftover < 16) return;
        poly1305_blocks(st, st->buf, 16, 1u << 24);
        st->leftover = 0;
    }
    if (bytes >= 16) {
        size_t want = bytes & ~(size_t)15;
        poly1305_blocks(st, m, want, 1u << 24);
        m += want;
        bytes -= want;
    }
    if (bytes) {
        memcpy(st->buf, m, bytes);
        st->leftover = bytes;
    }
}

static void poly1305_final(poly1305_ctx *st, uint8_t mac[16]) {
    if (st->leftover) {
        st->buf[st->leftover++] = 1;
        while (st->leftover < 16) st->buf[st->leftover++] = 0;
        poly1305_blocks(st, st->buf, 16, 0);
    }
    uint32_t h0 = st->h[0], h1 = st->h[1], h2 = st->h[2], h3 = st->h[3], h4 = st->h[4];
    uint32_t c;
    c = h1 >> 26; h1 &= 0x3ffffff; h2 += c;
    c = h2 >> 26; h2 &= 0x3ffffff; h3 += c;
    c = h3 >> 26; h3 &= 0x3ffffff; h4 += c;
    c = h4 >> 26; h4 &= 0x3ffffff; h0 += c * 5;
    c = h0 >> 26; h0 &= 0x3ffffff; h1 += c;

    uint32_t g0 = h0 + 5; c = g0 >> 26; g0 &= 0x3ffffff;
    uint32_t g1 = h1 + c; c = g1 >> 26; g1 &= 0x3ffffff;
    uint32_t g2 = h2 + c; c = g2 >> 26; g2 &= 0x3ffffff;
    uint32_t g3 = h3 + c; c = g3 >> 26; g3 &= 0x3ffffff;
    uint32_t g4 = h4 + c - (1u << 26);

    uint32_t mask = (g4 >> 31) - 1u;
    g0 &= mask; g1 &= mask; g2 &= mask; g3 &= mask; g4 &= mask;
    mask = ~mask;
    h0 = (h0 & mask) | g0; h1 = (h1 & mask) | g1;
    h2 = (h2 & mask) | g2; h3 = (h3 & mask) | g3; h4 = (h4 & mask) | g4;

    h0 = (h0) | (h1 << 26);
    h1 = (h1 >> 6) | (h2 << 20);
    h2 = (h2 >> 12) | (h3 << 14);
    h3 = (h3 >> 18) | (h4 << 8);

    uint64_t f;
    f = (uint64_t)h0 + st->pad[0];                h0 = (uint32_t)f;
    f = (uint64_t)h1 + st->pad[1] + (f >> 32);    h1 = (uint32_t)f;
    f = (uint64_t)h2 + st->pad[2] + (f >> 32);    h2 = (uint32_t)f;
    f = (uint64_t)h3 + st->pad[3] + (f >> 32);    h3 = (uint32_t)f;

    store32_le(mac + 0, h0);
    store32_le(mac + 4, h1);
    store32_le(mac + 8, h2);
    store32_le(mac + 12, h3);
}

static void poly1305_aead_mac(const uint8_t otk[32], const uint8_t *aad, size_t aad_len,
                              const uint8_t *ct, size_t ct_len, uint8_t tag[16]) {
    static const uint8_t zeros[16] = {0};
    poly1305_ctx st;
    poly1305_init(&st, otk);
    poly1305_update(&st, aad, aad_len);
    if (aad_len % 16) poly1305_update(&st, zeros, 16 - (aad_len % 16));
    poly1305_update(&st, ct, ct_len);
    if (ct_len % 16) poly1305_update(&st, zeros, 16 - (ct_len % 16));
    uint8_t lens[16];
    store64_le(lens, (uint64_t)aad_len);
    store64_le(lens + 8, (uint64_t)ct_len);
    poly1305_update(&st, lens, 16);
    poly1305_final(&st, tag);
}

static int ct_memcmp(const uint8_t *a, const uint8_t *b, size_t n) {
    uint8_t diff = 0;
    for (size_t i = 0; i < n; i++) diff |= a[i] ^ b[i];
    return diff != 0;
}

static void aead_chacha_seal(const uint8_t key[32], const uint8_t nonce[12],
                             const uint8_t *aad, size_t aad_len,
                             const uint8_t *pt, size_t pt_len,
                             uint8_t *out_ct_and_tag) {
    uint8_t otk[64];
    chacha20_block(key, 0, nonce, otk);
    chacha20_xor(key, 1, nonce, pt, out_ct_and_tag, pt_len);
    poly1305_aead_mac(otk, aad, aad_len, out_ct_and_tag, pt_len, out_ct_and_tag + pt_len);
}

static int aead_chacha_open(const uint8_t key[32], const uint8_t nonce[12],
                            const uint8_t *aad, size_t aad_len,
                            const uint8_t *in_ct_and_tag, size_t in_len,
                            uint8_t *out_pt) {
    if (in_len < ENIGMA_TAG_LEN) return -1;
    size_t ct_len = in_len - ENIGMA_TAG_LEN;
    uint8_t otk[64], expected[16];
    chacha20_block(key, 0, nonce, otk);
    poly1305_aead_mac(otk, aad, aad_len, in_ct_and_tag, ct_len, expected);
    if (ct_memcmp(expected, in_ct_and_tag + ct_len, 16) != 0) return -1;
    chacha20_xor(key, 1, nonce, in_ct_and_tag, out_pt, ct_len);
    return (int)ct_len;
}

/* ==================== Enigma v1 Protocol Core ==================== */
typedef struct {
    uint8_t raw[ENIGMA_KEYBOOK_LEN];
} enigma_keybook_t;

static int load_keybook(const char *path, enigma_keybook_t *kb) {
    FILE *f = fopen(path, "rb");
    if (!f) return -1;
    size_t n = fread(kb->raw, 1, ENIGMA_KEYBOOK_LEN, f);
    fclose(f);
    if (n != ENIGMA_KEYBOOK_LEN) return -1;
    if (memcmp(kb->raw, "NVENIGMA_KEY_V1\0", 16) != 0) return -1;
    return 0;
}

static void copy_corpus(const enigma_keybook_t *kb, size_t off, uint8_t *dst, size_t len) {
    const uint8_t *corpus = kb->raw + ENIGMA_CORPUS_OFF;
    size_t idx = off % ENIGMA_CORPUS_LEN;
    for (size_t i = 0; i < len; i++) {
        dst[i] = corpus[idx++];
        if (idx >= ENIGMA_CORPUS_LEN) idx = 0;
    }
}

static void compute_salt_mask(const enigma_keybook_t *kb, const uint8_t mimic[12], uint8_t mask[12]) {
    uint8_t buf[44], hash[32];
    memcpy(buf, kb->raw + ENIGMA_HINT_OFF, 32);
    memcpy(buf + 32, mimic, 12);
    sha256_once(buf, 44, hash);
    memcpy(mask, hash, 12);
}

static void derive_data_key(const enigma_keybook_t *kb, uint8_t r1, uint8_t r2, uint8_t r3,
                            uint32_t session_id, uint8_t out[32]) {
    uint8_t msg[14 + 32 * 3 + 4];
    memcpy(msg, "ENIGMA_V1_DATA", 14);
    memcpy(msg + 14, kb->raw + ENIGMA_ROTORS_OFF + (size_t)r1 * 32, 32);
    memcpy(msg + 46, kb->raw + ENIGMA_ROTORS_OFF + (size_t)r2 * 32, 32);
    memcpy(msg + 78, kb->raw + ENIGMA_ROTORS_OFF + (size_t)r3 * 32, 32);
    store32_be(msg + 110, session_id);
    hmac_sha256(kb->raw + ENIGMA_MASTER_OFF, 32, msg, sizeof(msg), out);
}

static void derive_rotor_and_hs_key(const enigma_keybook_t *kb, uint64_t slot, const uint8_t salt[12],
                                    uint8_t rot_hash[32], uint8_t *r1, uint8_t *r2, uint8_t *r3,
                                    uint8_t hs_key[32]) {
    uint8_t msg1[13 + 8 + 12];
    memcpy(msg1, "ENIGMA_V1_ROT", 13);
    store64_be(msg1 + 13, slot);
    memcpy(msg1 + 21, salt, 12);
    hmac_sha256(kb->raw + ENIGMA_MASTER_OFF, 32, msg1, sizeof(msg1), rot_hash);
    *r1 = rot_hash[0];
    *r2 = rot_hash[1];
    *r3 = rot_hash[2];

    uint8_t msg2[32 * 3 + 8 + 12];
    memcpy(msg2,      kb->raw + ENIGMA_ROTORS_OFF + (size_t)(*r1) * 32, 32);
    memcpy(msg2 + 32, kb->raw + ENIGMA_ROTORS_OFF + (size_t)(*r2) * 32, 32);
    memcpy(msg2 + 64, kb->raw + ENIGMA_ROTORS_OFF + (size_t)(*r3) * 32, 32);
    store64_be(msg2 + 96, slot);
    memcpy(msg2 + 104, salt, 12);
    hmac_sha256(kb->raw + ENIGMA_MASTER_OFF, 32, msg2, sizeof(msg2), hs_key);
}

static void build_mimic_header(uint8_t mode, const uint8_t r[8], size_t approx_len, uint8_t out[12]) {
    uint16_t len16 = (approx_len > 65535) ? 65535 : (uint16_t)approx_len;
    uint8_t hi = (uint8_t)(len16 >> 8), lo = (uint8_t)(len16 & 0xff);
    switch (mode & 3) {
        case 0:
            out[0] = (uint8_t)(0xC0 | (r[0] & 0x3F));
            out[1] = 0; out[2] = 0; out[3] = 0; out[4] = 1; out[5] = 8;
            memcpy(out + 6, r + 1, 6);
            break;
        case 1:
            out[0] = 0x00; out[1] = 0x01; out[2] = hi; out[3] = lo;
            out[4] = 0x21; out[5] = 0x12; out[6] = 0xA4; out[7] = 0x42;
            memcpy(out + 8, r, 4);
            break;
        case 2:
            out[0] = 0x17; out[1] = 0xFE; out[2] = 0xFD; out[3] = 0x00; out[4] = 0x01;
            memcpy(out + 5, r, 5);
            out[10] = hi; out[11] = lo;
            break;
        default:
            out[0] = r[0]; out[1] = r[1]; out[2] = 0x01; out[3] = 0x00;
            out[4] = 0x00; out[5] = 0x01; memset(out + 6, 0, 6);
            break;
    }
}

static int enigma_open_handshake(const enigma_keybook_t *kb, const uint8_t *wire, size_t wire_len,
                                 uint64_t now_secs, uint8_t *out_wg, uint32_t *out_sid,
                                 uint8_t out_salt[12], uint8_t out_data_key[32]) {
    if (wire_len < ENIGMA_HDR_LEN + 7 + ENIGMA_TAG_LEN || wire_len > MAX_PKT_BUF) return -1;
    uint8_t mask[12], salt[12];
    compute_salt_mask(kb, wire, mask);
    for (int i = 0; i < 12; i++) salt[i] = wire[12 + i] ^ mask[i];
    uint32_t sid = load32_be(salt);
    uint64_t cur_slot = now_secs / ENIGMA_TIME_SLOT;
    uint64_t slots[3] = { cur_slot, cur_slot > 0 ? cur_slot - 1 : 0, cur_slot + 1 };

    uint8_t plain[MAX_PKT_BUF];
    for (int s = 0; s < 3; s++) {
        uint8_t rot_hash[32], r1, r2, r3, hs_key[32];
        derive_rotor_and_hs_key(kb, slots[s], salt, rot_hash, &r1, &r2, &r3, hs_key);
        int pt_len = aead_chacha_open(hs_key, salt, wire, ENIGMA_HDR_LEN,
                                      wire + ENIGMA_HDR_LEN, wire_len - ENIGMA_HDR_LEN, plain);
        if (pt_len >= 7) {
            int swapped = (plain[0] & 1) != 0;
            size_t pre_len = plain[1];
            size_t mid_len = plain[2];
            size_t a_len = load16_be(plain + 3);
            size_t b_len = load16_be(plain + 5);
            size_t needed = 7 + pre_len + a_len + mid_len + b_len;
            if ((size_t)pt_len < needed || a_len + b_len == 0 || a_len + b_len > 2048) return -1;
            size_t p1 = 7 + pre_len;
            if (swapped) {
                const uint8_t *b_ptr = plain + p1;
                const uint8_t *a_ptr = plain + p1 + b_len + mid_len;
                memcpy(out_wg, a_ptr, a_len);
                memcpy(out_wg + a_len, b_ptr, b_len);
            } else {
                const uint8_t *a_ptr = plain + p1;
                const uint8_t *b_ptr = plain + p1 + a_len + mid_len;
                memcpy(out_wg, a_ptr, a_len);
                memcpy(out_wg + a_len, b_ptr, b_len);
            }
            *out_sid = sid;
            memcpy(out_salt, salt, 12);
            derive_data_key(kb, r1, r2, r3, sid, out_data_key);
            return (int)(a_len + b_len);
        }
    }
    return -1;
}

static int enigma_seal_handshake(const enigma_keybook_t *kb, const uint8_t *wg, size_t wg_len,
                                 uint32_t session_id, const uint8_t rand8[8], uint64_t now_secs,
                                 uint8_t *out_wire) {
    if (wg_len < 4 || wg_len > 2048) return -1;
    uint8_t salt[12];
    store32_be(salt, session_id);
    memcpy(salt + 4, rand8, 8);
    uint64_t slot = now_secs / ENIGMA_TIME_SLOT;
    uint8_t rot_hash[32], r1, r2, r3, hs_key[32];
    derive_rotor_and_hs_key(kb, slot, salt, rot_hash, &r1, &r2, &r3, hs_key);

    int swap_flag = (rot_hash[4] & 1) != 0;
    size_t pre_len = rot_hash[5] % 96;
    size_t mid_len = rot_hash[6] % 96;
    size_t post_len = 16 + (rot_hash[7] % 140);
    size_t split_pos = (wg_len >= 32) ? (16 + (rot_hash[3] % (wg_len - 31))) : (wg_len / 2);
    size_t a_len = split_pos, b_len = wg_len - split_pos;
    size_t inner_len = 7 + pre_len + wg_len + mid_len + post_len;
    if (ENIGMA_HDR_LEN + inner_len + ENIGMA_TAG_LEN > MAX_PKT_BUF) return -1;

    uint8_t mimic[12], mask[12];
    build_mimic_header(rot_hash[11] & 3, rand8, 12 + inner_len + ENIGMA_TAG_LEN, mimic);
    compute_salt_mask(kb, mimic, mask);
    memcpy(out_wire, mimic, 12);
    for (int i = 0; i < 12; i++) out_wire[12 + i] = salt[i] ^ mask[i];

    uint8_t body[MAX_PKT_BUF];
    body[0] = (uint8_t)((swap_flag ? 1 : 0) | (rot_hash[8] & 0xFE));
    body[1] = (uint8_t)pre_len;
    body[2] = (uint8_t)mid_len;
    store16_be(body + 3, (uint16_t)a_len);
    store16_be(body + 5, (uint16_t)b_len);

    size_t c0 = load16_be(rot_hash + 9);
    size_t off = 7;
    if (pre_len) { copy_corpus(kb, c0, body + off, pre_len); off += pre_len; }
    if (swap_flag) { memcpy(body + off, wg + split_pos, b_len); off += b_len; }
    else           { memcpy(body + off, wg, a_len); off += a_len; }
    if (mid_len) { copy_corpus(kb, c0 + 137, body + off, mid_len); off += mid_len; }
    if (swap_flag) { memcpy(body + off, wg, a_len); off += a_len; }
    else           { memcpy(body + off, wg + split_pos, b_len); off += b_len; }
    if (post_len) { copy_corpus(kb, c0 + 509, body + off, post_len); off += post_len; }

    aead_chacha_seal(hs_key, salt, out_wire, ENIGMA_HDR_LEN, body, off, out_wire + ENIGMA_HDR_LEN);
    return (int)(ENIGMA_HDR_LEN + off + ENIGMA_TAG_LEN);
}

static int enigma_open_data(const enigma_keybook_t *kb, const uint8_t data_key[32],
                            const uint8_t *wire, size_t wire_len,
                            uint8_t *out_wg, uint32_t *out_sid, uint64_t *out_seq) {
    if (wire_len < ENIGMA_HDR_LEN + 2 + ENIGMA_TAG_LEN || wire_len > MAX_PKT_BUF) return -1;
    uint8_t mask[12], salt[12];
    compute_salt_mask(kb, wire, mask);
    for (int i = 0; i < 12; i++) salt[i] = wire[12 + i] ^ mask[i];

    uint8_t plain[MAX_PKT_BUF];
    int pt_len = aead_chacha_open(data_key, salt, wire, ENIGMA_HDR_LEN,
                                  wire + ENIGMA_HDR_LEN, wire_len - ENIGMA_HDR_LEN, plain);
    if (pt_len < 2) return -1;
    size_t orig_len = load16_be(plain);
    if (orig_len == 0 || (size_t)pt_len < 2 + orig_len) return -1;
    memcpy(out_wg, plain + 2, orig_len);
    *out_sid = load32_be(salt);
    uint64_t seq = 0;
    for (int i = 0; i < 8; i++) seq = (seq << 8) | salt[4 + i];
    *out_seq = seq;
    return (int)orig_len;
}

static int enigma_seal_data(const enigma_keybook_t *kb, const uint8_t data_key[32],
                            uint32_t session_id, uint64_t seq,
                            const uint8_t *wg, size_t wg_len, uint8_t *out_wire) {
    if (wg_len > 2048) return -1;
    uint8_t salt[12];
    store32_be(salt, session_id);
    store64_be(salt + 4, seq);

    uint8_t rot_idx = (uint8_t)((seq & 0xff) ^ (session_id & 0xff));
    uint8_t rot_byte = kb->raw[ENIGMA_ROTORS_OFF + (size_t)rot_idx * 32 + ((seq >> 8) & 31)];
    size_t pad_len;
    if (wg_len <= 128)       pad_len = 24 + (rot_byte % 120);
    else if (wg_len <= 1100) pad_len = 8 + (rot_byte % 32);
    else                     pad_len = rot_byte % 12;

    size_t inner_len = 2 + wg_len + pad_len;
    if (ENIGMA_HDR_LEN + inner_len + ENIGMA_TAG_LEN > MAX_PKT_BUF) return -1;

    uint8_t mimic[12], mask[12];
    build_mimic_header((rot_byte >> 2) & 3, salt + 4, 12 + inner_len + ENIGMA_TAG_LEN, mimic);
    compute_salt_mask(kb, mimic, mask);
    memcpy(out_wire, mimic, 12);
    for (int i = 0; i < 12; i++) out_wire[12 + i] = salt[i] ^ mask[i];

    uint8_t body[MAX_PKT_BUF];
    store16_be(body, (uint16_t)wg_len);
    memcpy(body + 2, wg, wg_len);
    if (pad_len) {
        size_t c_off = (size_t)seq * 37u + (size_t)rot_byte;
        copy_corpus(kb, c_off, body + 2 + wg_len, pad_len);
    }
    aead_chacha_seal(data_key, salt, out_wire, ENIGMA_HDR_LEN, body, inner_len, out_wire + ENIGMA_HDR_LEN);
    return (int)(ENIGMA_HDR_LEN + inner_len + ENIGMA_TAG_LEN);
}

#ifndef ENIGMA_SELF_TEST
typedef struct {
    uint8_t salt[12];
    uint64_t ts;
} replay_entry_t;

static replay_entry_t g_replay[REPLAY_CAP];
static size_t g_replay_head = 0;
static size_t g_replay_count = 0;

static int replay_check_insert(const uint8_t salt[12], uint64_t now) {
    for (size_t i = 0; i < g_replay_count; i++) {
        size_t idx = (g_replay_head + REPLAY_CAP - 1 - i) % REPLAY_CAP;
        if (now > g_replay[idx].ts && now - g_replay[idx].ts > 120) break;
        if (memcmp(g_replay[idx].salt, salt, 12) == 0) return 0;
    }
    memcpy(g_replay[g_replay_head].salt, salt, 12);
    g_replay[g_replay_head].ts = now;
    g_replay_head = (g_replay_head + 1) % REPLAY_CAP;
    if (g_replay_count < REPLAY_CAP) g_replay_count++;
    return 1;
}

typedef struct {
    int active;
    uint32_t session_id;
    struct sockaddr_in client_addr;
    int upstream_fd;
    uint8_t cur_data_key[32];
    uint8_t prev_data_key[32];
    int has_prev_key;
    uint64_t tx_seq;
    uint64_t last_seen;
} enigma_session_t;

static enigma_session_t g_sessions[MAX_SESSIONS];
static volatile sig_atomic_t g_running = 1;
static void on_sig(int sig) { (void)sig; g_running = 0; }

static void fill_rand(uint8_t *buf, size_t n) {
    int fd = open("/dev/urandom", O_RDONLY);
    if (fd >= 0) {
        ssize_t r = read(fd, buf, n);
        (void)r;
        close(fd);
    } else {
        uint64_t t = (uint64_t)time(NULL);
        for (size_t i = 0; i < n; i++) buf[i] = (uint8_t)((t >> ((i % 8) * 8)) ^ (i * 37));
    }
}

static int set_nonblock(int fd) {
    int fl = fcntl(fd, F_GETFL, 0);
    return (fl < 0) ? -1 : fcntl(fd, F_SETFL, fl | O_NONBLOCK);
}

static enigma_session_t *find_session(uint32_t sid) {
    for (int i = 0; i < MAX_SESSIONS; i++) {
        if (g_sessions[i].active && g_sessions[i].session_id == sid) return &g_sessions[i];
    }
    return NULL;
}

static enigma_session_t *alloc_session(int epfd, uint32_t sid, const struct sockaddr_in *caddr,
                                       uint16_t wg_port, uint64_t now) {
    int slot = -1;
    uint64_t oldest = UINT64_MAX;
    int oldest_idx = 0;
    for (int i = 0; i < MAX_SESSIONS; i++) {
        if (!g_sessions[i].active) { slot = i; break; }
        if (g_sessions[i].last_seen < oldest) { oldest = g_sessions[i].last_seen; oldest_idx = i; }
    }
    if (slot < 0) {
        slot = oldest_idx;
        epoll_ctl(epfd, EPOLL_CTL_DEL, g_sessions[slot].upstream_fd, NULL);
        close(g_sessions[slot].upstream_fd);
    }

    int ufd = socket(AF_INET, SOCK_DGRAM, 0);
    if (ufd < 0) return NULL;
    set_nonblock(ufd);

    struct sockaddr_in dst;
    memset(&dst, 0, sizeof(dst));
    dst.sin_family = AF_INET;
    dst.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    dst.sin_port = htons(wg_port);
    if (connect(ufd, (struct sockaddr *)&dst, sizeof(dst)) < 0) {
        close(ufd);
        return NULL;
    }

    struct epoll_event ev;
    memset(&ev, 0, sizeof(ev));
    ev.events = EPOLLIN;
    ev.data.u32 = (uint32_t)(slot + 1); /* 0 reserved for ext_fd */
    if (epoll_ctl(epfd, EPOLL_CTL_ADD, ufd, &ev) < 0) {
        close(ufd);
        return NULL;
    }

    memset(&g_sessions[slot], 0, sizeof(enigma_session_t));
    g_sessions[slot].active = 1;
    g_sessions[slot].session_id = sid;
    g_sessions[slot].client_addr = *caddr;
    g_sessions[slot].upstream_fd = ufd;
    g_sessions[slot].tx_seq = 1;
    g_sessions[slot].last_seen = now;
    return &g_sessions[slot];
}

int main(int argc, char **argv) {
    if (argc != 4) {
        fprintf(stderr, "Usage: %s <listen_port> <wg_upstream_port> <keybook.nvkey>\n", argv[0]);
        return 1;
    }
    uint16_t listen_port = (uint16_t)atoi(argv[1]);
    uint16_t wg_port = (uint16_t)atoi(argv[2]);
    enigma_keybook_t kb;
    if (load_keybook(argv[3], &kb) != 0) {
        fprintf(stderr, "Failed to load keybook: %s\n", argv[3]);
        return 1;
    }

    signal(SIGINT, on_sig);
    signal(SIGTERM, on_sig);
    memset(g_sessions, 0, sizeof(g_sessions));

    int ext_fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (ext_fd < 0) { perror("socket"); return 1; }
    int one = 1;
    setsockopt(ext_fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    int rbuf = 4 * 1024 * 1024;
    setsockopt(ext_fd, SOL_SOCKET, SO_RCVBUF, &rbuf, sizeof(rbuf));
    setsockopt(ext_fd, SOL_SOCKET, SO_SNDBUF, &rbuf, sizeof(rbuf));
    set_nonblock(ext_fd);

    struct sockaddr_in bind_addr;
    memset(&bind_addr, 0, sizeof(bind_addr));
    bind_addr.sin_family = AF_INET;
    bind_addr.sin_addr.s_addr = htonl(INADDR_ANY);
    bind_addr.sin_port = htons(listen_port);
    if (bind(ext_fd, (struct sockaddr *)&bind_addr, sizeof(bind_addr)) < 0) {
        perror("bind"); close(ext_fd); return 1;
    }

    int epfd = epoll_create1(0);
    if (epfd < 0) { perror("epoll"); close(ext_fd); return 1; }
    struct epoll_event ev;
    memset(&ev, 0, sizeof(ev));
    ev.events = EPOLLIN;
    ev.data.u32 = 0;
    epoll_ctl(epfd, EPOLL_CTL_ADD, ext_fd, &ev);

    printf("neuroveil-enigma-srv listening UDP :%u -> 127.0.0.1:%u\n", listen_port, wg_port);
    fflush(stdout);

    struct epoll_event events[64];
    uint8_t buf[MAX_PKT_BUF], wg_out[MAX_PKT_BUF], wire_out[MAX_PKT_BUF];
    uint64_t last_gc = (uint64_t)time(NULL);

    while (g_running) {
        int nfds = epoll_wait(epfd, events, 64, 5000);
        uint64_t now = (uint64_t)time(NULL);

        for (int i = 0; i < nfds; i++) {
            uint32_t tag = events[i].data.u32;
            if (tag == 0) {
                for (;;) {
                    struct sockaddr_in cli;
                    socklen_t clen = sizeof(cli);
                    ssize_t n = recvfrom(ext_fd, buf, sizeof(buf), 0, (struct sockaddr *)&cli, &clen);
                    if (n <= 0) break;
                    if (n < ENIGMA_HDR_LEN + 2 + ENIGMA_TAG_LEN) continue;

                    uint8_t mask[12], salt[12];
                    compute_salt_mask(&kb, buf, mask);
                    for (int k = 0; k < 12; k++) salt[k] = buf[12 + k] ^ mask[k];
                    uint32_t sid = load32_be(salt);

                    int handled = 0;
                    enigma_session_t *sess = find_session(sid);
                    if (sess) {
                        uint32_t dsid = 0; uint64_t dseq = 0;
                        int wg_len = enigma_open_data(&kb, sess->cur_data_key, buf, (size_t)n, wg_out, &dsid, &dseq);
                        if (wg_len < 0 && sess->has_prev_key) {
                            wg_len = enigma_open_data(&kb, sess->prev_data_key, buf, (size_t)n, wg_out, &dsid, &dseq);
                        }
                        if (wg_len > 0 && dsid == sid) {
                            sess->client_addr = cli;
                            sess->last_seen = now;
                            ssize_t sr = send(sess->upstream_fd, wg_out, (size_t)wg_len, 0);
                            (void)sr;
                            handled = 1;
                        }
                    }
                    if (!handled) {
                        uint32_t hsid = 0;
                        uint8_t hsalt[12], new_dk[32];
                        int wg_len = enigma_open_handshake(&kb, buf, (size_t)n, now, wg_out, &hsid, hsalt, new_dk);
                        if (wg_len > 0 && replay_check_insert(hsalt, now)) {
                            enigma_session_t *s = find_session(hsid);
                            if (!s) s = alloc_session(epfd, hsid, &cli, wg_port, now);
                            if (s) {
                                s->client_addr = cli;
                                s->last_seen = now;
                                if (wg_out[0] == 1 || !s->has_prev_key) {
                                    memcpy(s->prev_data_key, s->cur_data_key, 32);
                                    s->has_prev_key = 1;
                                    memcpy(s->cur_data_key, new_dk, 32);
                                }
                                ssize_t sr = send(s->upstream_fd, wg_out, (size_t)wg_len, 0);
                                (void)sr;
                            }
                        }
                    }
                }
            } else {
                int sidx = (int)(tag - 1);
                if (sidx >= 0 && sidx < MAX_SESSIONS && g_sessions[sidx].active) {
                    enigma_session_t *s = &g_sessions[sidx];
                    for (;;) {
                        ssize_t n = recv(s->upstream_fd, buf, sizeof(buf), 0);
                        if (n <= 0) break;
                        s->last_seen = now;
                        int is_hs = (n >= 4 && buf[1] == 0 && buf[2] == 0 && buf[3] == 0 && (buf[0] == 1 || buf[0] == 2 || buf[0] == 3));
                        int wlen;
                        if (is_hs) {
                            uint8_t rand8[8];
                            fill_rand(rand8, 8);
                            wlen = enigma_seal_handshake(&kb, buf, (size_t)n, s->session_id, rand8, now, wire_out);
                        } else {
                            uint64_t seq = s->tx_seq++;
                            wlen = enigma_seal_data(&kb, s->cur_data_key, s->session_id, seq, buf, (size_t)n, wire_out);
                        }
                        if (wlen > 0) {
                            ssize_t sr = sendto(ext_fd, wire_out, (size_t)wlen, 0,
                                                (struct sockaddr *)&s->client_addr, sizeof(s->client_addr));
                            (void)sr;
                        }
                    }
                }
            }
        }

        if (now - last_gc >= 30) {
            last_gc = now;
            for (int i = 0; i < MAX_SESSIONS; i++) {
                if (g_sessions[i].active && now - g_sessions[i].last_seen > 300) {
                    epoll_ctl(epfd, EPOLL_CTL_DEL, g_sessions[i].upstream_fd, NULL);
                    close(g_sessions[i].upstream_fd);
                    g_sessions[i].active = 0;
                }
            }
        }
    }

    for (int i = 0; i < MAX_SESSIONS; i++) {
        if (g_sessions[i].active) close(g_sessions[i].upstream_fd);
    }
    close(epfd);
    close(ext_fd);
    return 0;
}
#else
/* Cross-verification CLI test mode (compiled with -DENIGMA_SELF_TEST on Windows/Linux):
 * Usage:
 *   self_test <keybook.nvkey> <hs_in.bin> <data_in.bin> <now_secs> [hs_c_out.bin data_c_out.bin]
 */
int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "Usage: self_test <keybook.nvkey> [hs_in.bin data_in.bin now_secs [hs_c_out.bin data_c_out.bin]]\n"); return 1; }
    enigma_keybook_t kb;
    if (load_keybook(argv[1], &kb) != 0) { fprintf(stderr, "BAD_KEYBOOK\n"); return 2; }
    if (argc >= 5) {
        FILE *fhs = fopen(argv[2], "rb");
        FILE *fdt = fopen(argv[3], "rb");
        if (!fhs || !fdt) return 3;
        uint8_t hs_buf[MAX_PKT_BUF], dt_buf[MAX_PKT_BUF], out[MAX_PKT_BUF];
        size_t hs_len = fread(hs_buf, 1, sizeof(hs_buf), fhs);
        size_t dt_len = fread(dt_buf, 1, sizeof(dt_buf), fdt);
        fclose(fhs); fclose(fdt);
        uint64_t now = (uint64_t)strtoull(argv[4], NULL, 10);
        uint32_t sid = 0; uint8_t salt[12], dk[32], wg[MAX_PKT_BUF];
        int w1 = enigma_open_handshake(&kb, hs_buf, hs_len, now, wg, &sid, salt, dk);
        if (w1 <= 0) { fprintf(stderr, "HS_OPEN_FAIL\n"); return 4; }
        uint32_t dsid = 0; uint64_t dseq = 0;
        int w2 = enigma_open_data(&kb, dk, dt_buf, dt_len, out, &dsid, &dseq);
        if (w2 <= 0) { fprintf(stderr, "DATA_OPEN_FAIL\n"); return 5; }

        /* Also test sealing from C and roundtrip opening in C */
        uint8_t c_hs[MAX_PKT_BUF], c_dt[MAX_PKT_BUF];
        uint8_t rand8[8] = {0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88};
        int c_hs_len = enigma_seal_handshake(&kb, wg, (size_t)w1, sid, rand8, now, c_hs);
        if (c_hs_len <= 0) { fprintf(stderr, "C_HS_SEAL_FAIL\n"); return 6; }
        int c_dt_len = enigma_seal_data(&kb, dk, sid, dseq + 1, out, (size_t)w2, c_dt);
        if (c_dt_len <= 0) { fprintf(stderr, "C_DT_SEAL_FAIL\n"); return 7; }

        if (argc >= 7) {
            FILE *fo1 = fopen(argv[5], "wb");
            FILE *fo2 = fopen(argv[6], "wb");
            if (fo1 && fo2) {
                fwrite(c_hs, 1, (size_t)c_hs_len, fo1);
                fwrite(c_dt, 1, (size_t)c_dt_len, fo2);
                fclose(fo1); fclose(fo2);
            }
        }
        printf("C_CROSS_OK sid=%08X wg_hs=%d wg_dt=%d seq=%llu c_hs=%d c_dt=%d\n",
               sid, w1, w2, (unsigned long long)dseq, c_hs_len, c_dt_len);
        return 0;
    }
    return 0;
}
#endif
