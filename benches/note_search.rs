use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use emergence::data::{notes::NoteSearch, shove_test_data, ConnectionType, Store};

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("Database");
    group.sample_size(1000);
    group.noise_threshold(0.05);

    let store = Store::new(ConnectionType::InMemory);
    shove_test_data(&mut *store.conn.borrow_mut(), 10_000).unwrap();

    let subjects = &store.get_subjects().unwrap();
    let subject = subjects.first().unwrap();

    group.bench_function("Read all notes", |b| {
        b.iter(|| store.get_notes(NoteSearch::default()))
    });

    group.bench_function("Read notes from subject", |b| {
        b.iter(|| {
            store.get_notes(NoteSearch {
                subject_id: Some(subject.id),
                ..Default::default()
            })
        })
    });

    group.bench_function("Read all tasks", |b| {
        b.iter(|| {
            store.get_notes(NoteSearch {
                task_only: true,
                ..Default::default()
            })
        })
    });

    group.bench_function("Read tasks from subject", |b| {
        b.iter(|| {
            store.get_notes(NoteSearch {
                subject_id: Some(subject.id),
                task_only: true,
                ..Default::default()
            })
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets = criterion_benchmark
}

criterion_main!(benches);
