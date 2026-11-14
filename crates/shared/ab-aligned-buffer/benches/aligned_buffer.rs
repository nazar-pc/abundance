use ab_aligned_buffer::alignment::{Align1, Align16, Align4096};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    let bytes = [1; 1024];

    c.bench_function("owned/with-capacity/64", |b| {
        b.iter(|| {
            black_box(OwnedAlignedBuffer::<Align16>::with_capacity(black_box(64)));
        });
    });
    c.bench_function("owned/with-capacity-aligned/4096", |b| {
        b.iter(|| {
            black_box(OwnedAlignedBuffer::<Align1>::with_capacity_aligned::<
                Align4096,
            >(black_box(4096)));
        });
    });
    c.bench_function("owned/from-bytes/64", |b| {
        b.iter(|| {
            black_box(OwnedAlignedBuffer::<Align16>::from_bytes(black_box(
                &bytes[..64],
            )));
        });
    });
    c.bench_function("owned/append/8-bytes-x-128", |b| {
        let mut owned = OwnedAlignedBuffer::<Align16>::with_capacity(1024);
        b.iter(|| {
            owned.truncate(0);
            for chunk in bytes.as_chunks::<8>().0 {
                assert!(owned.append(black_box(chunk)));
            }
        });
    });
    c.bench_function("owned/append-with-growth/8-bytes-x-128", |b| {
        b.iter(|| {
            let mut owned = OwnedAlignedBuffer::<Align16>::EMPTY;
            for chunk in bytes.as_chunks::<8>().0 {
                assert!(owned.append(black_box(chunk)));
            }
            black_box(owned);
        });
    });
    c.bench_function("owned/clone/64", |b| {
        let owned = OwnedAlignedBuffer::<Align16>::from_bytes(&bytes[..64]);
        b.iter(|| {
            black_box(black_box(&owned).clone());
        });
    });
    c.bench_function("owned/into-shared-into-owned/64", |b| {
        let mut owned = Some(OwnedAlignedBuffer::<Align16>::from_bytes(&bytes[..64]));
        b.iter(|| {
            let shared = black_box(owned.take().unwrap().into_shared());
            owned = Some(shared.into_owned());
        });
    });

    let shared = SharedAlignedBuffer::<Align16>::from_bytes(&bytes);
    c.bench_function("shared/clone-drop", |b| {
        b.iter(|| {
            drop(black_box(black_box(&shared).clone()));
        });
    });
    let empty = SharedAlignedBuffer::<Align16>::EMPTY;
    c.bench_function("shared/clone-drop-empty", |b| {
        b.iter(|| {
            drop(black_box(black_box(&empty).clone()));
        });
    });
    c.bench_function("shared/get-slice-drop", |b| {
        b.iter(|| {
            drop(black_box(black_box(&shared).get_slice(black_box(8..72))));
        });
    });
    c.bench_function("shared/as-slice", |b| {
        b.iter(|| {
            black_box(black_box(&shared).as_slice());
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
