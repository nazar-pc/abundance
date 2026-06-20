#[cfg(not(miri))]
use crate::utils::parse_cpu_cores_sets;
// TODO: Not supported on Miri on macOS yet: https://github.com/rust-lang/miri/issues/4007
#[cfg(not(all(miri, target_os = "macos")))]
// TODO: Not supported on Miri on Windows yet: https://github.com/rust-lang/miri/issues/1719
#[cfg(not(all(miri, target_os = "windows")))]
use crate::utils::run_future_in_dedicated_thread;
use crate::utils::{CpuCoreSet, thread_pool_core_indices_internal};
use gdt_cpus::AffinityMask;
// TODO: Not supported on Miri on macOS yet: https://github.com/rust-lang/miri/issues/4007
#[cfg(not(all(miri, target_os = "macos")))]
// TODO: Not supported on Miri on Windows yet: https://github.com/rust-lang/miri/issues/1719
#[cfg(not(all(miri, target_os = "windows")))]
use std::future;
use std::num::NonZeroUsize;
// TODO: Not supported on Miri on macOS yet: https://github.com/rust-lang/miri/issues/4007
#[cfg(not(all(miri, target_os = "macos")))]
// TODO: Not supported on Miri on Windows yet: https://github.com/rust-lang/miri/issues/1719
#[cfg(not(all(miri, target_os = "windows")))]
use tokio::sync::oneshot;

// TODO: Not supported on Miri on macOS yet: https://github.com/rust-lang/miri/issues/4007
#[cfg(not(all(miri, target_os = "macos")))]
// TODO: Not supported on Miri on Windows yet: https://github.com/rust-lang/miri/issues/1719
#[cfg(not(all(miri, target_os = "windows")))]
#[tokio::test]
async fn run_future_in_dedicated_thread_ready() {
    let value = run_future_in_dedicated_thread(|| future::ready(1u8), "ready".to_string())
        .unwrap()
        .await
        .unwrap();

    assert_eq!(value, 1);
}

// TODO: Not supported on Miri on macOS yet: https://github.com/rust-lang/miri/issues/4007
#[cfg(not(all(miri, target_os = "macos")))]
// TODO: Not supported on Miri on Windows yet: https://github.com/rust-lang/miri/issues/1719
#[cfg(not(all(miri, target_os = "windows")))]
#[tokio::test]
async fn run_future_in_dedicated_thread_cancellation() {
    // This may hang if not implemented correctly
    drop(
        run_future_in_dedicated_thread(future::pending::<()>, "cancellation".to_string()).unwrap(),
    );
}

// TODO: Not supported on Miri on macOS yet: https://github.com/rust-lang/miri/issues/4007
#[cfg(not(all(miri, target_os = "macos")))]
// TODO: Not supported on Miri on Windows yet: https://github.com/rust-lang/miri/issues/1719
#[cfg(not(all(miri, target_os = "windows")))]
#[test]
fn run_future_in_dedicated_thread_tokio_on_drop() {
    struct S;

    impl Drop for S {
        fn drop(&mut self) {
            // This will panic only if called from non-tokio thread
            tokio::task::spawn_blocking(|| {
                // Nothing
            });
        }
    }

    let (_sender, receiver) = oneshot::channel::<()>();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        drop(run_future_in_dedicated_thread(
            move || async move {
                let s = S;
                let _: Result<(), _> = receiver.await;
                drop(s);
            },
            "tokio_on_drop".to_string(),
        ));
    });
}

#[cfg(not(miri))]
#[test]
fn test_parse_cpu_cores_sets() {
    {
        let cores = parse_cpu_cores_sets("0").unwrap();
        assert_eq!(cores.len(), 1);
        assert_eq!(cores[0].affinity_mask, AffinityMask::from_iter([0]));
    }
    {
        let cores = parse_cpu_cores_sets("0,1,2").unwrap();
        assert_eq!(cores.len(), 1);
        assert_eq!(cores[0].affinity_mask, AffinityMask::from_iter([0, 1, 2]));
    }
    {
        let cores = parse_cpu_cores_sets("0,1,2 4,5,6").unwrap();
        assert_eq!(cores.len(), 2);
        assert_eq!(cores[0].affinity_mask, AffinityMask::from_iter([0, 1, 2]));
        assert_eq!(cores[1].affinity_mask, AffinityMask::from_iter([4, 5, 6]));
    }
    {
        let cores = parse_cpu_cores_sets("0-2 4-6,7").unwrap();
        assert_eq!(cores.len(), 2);
        assert_eq!(cores[0].affinity_mask, AffinityMask::from_iter([0, 1, 2]));
        assert_eq!(
            cores[1].affinity_mask,
            AffinityMask::from_iter([4, 5, 6, 7])
        );
    }

    parse_cpu_cores_sets("").unwrap_err();
    parse_cpu_cores_sets("a").unwrap_err();
    parse_cpu_cores_sets("0,").unwrap_err();
    parse_cpu_cores_sets("0,a").unwrap_err();
    parse_cpu_cores_sets("0 a").unwrap_err();
}

#[test]
fn test_thread_pool_core_indices() {
    let all_cpu_cores = vec![
        CpuCoreSet {
            affinity_mask: AffinityMask::from_iter([0, 1]),
            cpu_info: None,
        },
        CpuCoreSet {
            affinity_mask: AffinityMask::from_iter([4, 5]),
            cpu_info: None,
        },
        CpuCoreSet {
            affinity_mask: AffinityMask::from_iter([2, 3]),
            cpu_info: None,
        },
        CpuCoreSet {
            affinity_mask: AffinityMask::from_iter([6, 7]),
            cpu_info: None,
        },
    ];

    // Default behavior
    assert_eq!(
        thread_pool_core_indices_internal(all_cpu_cores.clone(), None, None)
            .into_iter()
            .map(|cpu_core_set| cpu_core_set.affinity_mask)
            .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0, 1]),
            AffinityMask::from_iter([4, 5]),
            AffinityMask::from_iter([2, 3]),
            AffinityMask::from_iter([6, 7])
        ]
    );

    // Custom number of thread pools
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            None,
            Some(NonZeroUsize::new(1).unwrap())
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![AffinityMask::from_iter([0, 1, 4, 5, 2, 3, 6, 7])]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            None,
            Some(NonZeroUsize::new(2).unwrap())
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0, 1, 4, 5]),
            AffinityMask::from_iter([2, 3, 6, 7])
        ]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            None,
            Some(NonZeroUsize::new(3).unwrap())
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0, 1, 4]),
            AffinityMask::from_iter([5, 2, 3]),
            AffinityMask::from_iter([6, 7])
        ]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            None,
            Some(NonZeroUsize::new(4).unwrap())
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0, 1]),
            AffinityMask::from_iter([4, 5]),
            AffinityMask::from_iter([2, 3]),
            AffinityMask::from_iter([6, 7])
        ]
    );

    // Custom thread pool size
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            Some(NonZeroUsize::new(1).unwrap()),
            None,
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0]),
            AffinityMask::from_iter([1]),
            AffinityMask::from_iter([4]),
            AffinityMask::from_iter([5])
        ]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            Some(NonZeroUsize::new(2).unwrap()),
            None,
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0, 1]),
            AffinityMask::from_iter([4, 5]),
            AffinityMask::from_iter([2, 3]),
            AffinityMask::from_iter([6, 7])
        ]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            Some(NonZeroUsize::new(3).unwrap()),
            None,
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0, 1, 4]),
            AffinityMask::from_iter([5, 2, 3]),
            AffinityMask::from_iter([6, 7, 0]),
            AffinityMask::from_iter([1, 4, 5])
        ]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            Some(NonZeroUsize::new(4).unwrap()),
            None,
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0, 1, 4, 5]),
            AffinityMask::from_iter([2, 3, 6, 7]),
            AffinityMask::from_iter([0, 1, 4, 5]),
            AffinityMask::from_iter([2, 3, 6, 7])
        ]
    );

    // Custom number of thread pools and thread pool size
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            Some(NonZeroUsize::new(1).unwrap()),
            Some(NonZeroUsize::new(1).unwrap()),
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![AffinityMask::from_iter([0])]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            Some(NonZeroUsize::new(2).unwrap()),
            Some(NonZeroUsize::new(4).unwrap()),
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0, 1]),
            AffinityMask::from_iter([4, 5]),
            AffinityMask::from_iter([2, 3]),
            AffinityMask::from_iter([6, 7])
        ]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            Some(NonZeroUsize::new(8).unwrap()),
            Some(NonZeroUsize::new(1).unwrap()),
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![AffinityMask::from_iter([0, 1, 4, 5, 2, 3, 6, 7])]
    );
    assert_eq!(
        thread_pool_core_indices_internal(
            all_cpu_cores.clone(),
            Some(NonZeroUsize::new(1).unwrap()),
            Some(NonZeroUsize::new(8).unwrap()),
        )
        .into_iter()
        .map(|cpu_core_set| cpu_core_set.affinity_mask)
        .collect::<Vec<_>>(),
        vec![
            AffinityMask::from_iter([0]),
            AffinityMask::from_iter([1]),
            AffinityMask::from_iter([4]),
            AffinityMask::from_iter([5]),
            AffinityMask::from_iter([2]),
            AffinityMask::from_iter([3]),
            AffinityMask::from_iter([6]),
            AffinityMask::from_iter([7])
        ]
    );
}
