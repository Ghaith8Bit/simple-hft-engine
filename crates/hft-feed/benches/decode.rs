use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hft_feed::decode::decode;
use hft_feed::replay::{encode_add, encode_cancel, generate_stream};
use hft_feed::wire::{AddOrderMsg, MSG_SIZE};
use zerocopy::FromBytes;

fn bench_decode_add(c: &mut Criterion) {
    let buf = encode_add(1, 0, 100, 10);
    c.bench_function("decode_add", |b| {
        b.iter(|| {
            let msg = decode(black_box(buf.as_slice())).unwrap();
            black_box(msg);
        });
    });
}

fn bench_decode_cancel(c: &mut Criterion) {
    let buf = encode_cancel(1, 0, 100, 10);
    c.bench_function("decode_cancel", |b| {
        b.iter(|| {
            let msg = decode(black_box(buf.as_slice())).unwrap();
            black_box(msg);
        });
    });
}

fn bench_decode_stream_1000(c: &mut Criterion) {
    let stream = generate_stream(1000);
    c.bench_function("decode_stream_1000", |b| {
        b.iter(|| {
            let mut count = 0u32;
            let mut offset = 0;
            while offset + MSG_SIZE <= stream.len() {
                if decode(&stream[offset..offset + MSG_SIZE]).is_ok() {
                    count += 1;
                }
                offset += MSG_SIZE;
            }
            black_box(count);
        });
    });
}

fn bench_ref_from_bytes_floor(c: &mut Criterion) {
    // Theoretical floor: one ref_from_bytes, no dispatch on message type.
    let buf = encode_add(1, 0, 100, 10);
    c.bench_function("ref_from_bytes_add_floor", |b| {
        b.iter(|| {
            let msg = AddOrderMsg::ref_from_bytes(black_box(buf.as_slice())).unwrap();
            black_box(msg);
        });
    });
}

criterion_group!(
    benches,
    bench_decode_add,
    bench_decode_cancel,
    bench_decode_stream_1000,
    bench_ref_from_bytes_floor,
);
criterion_main!(benches);
