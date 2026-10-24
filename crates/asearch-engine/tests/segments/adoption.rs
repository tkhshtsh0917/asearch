//! Atomic staging, cancellation and adoption lifetime.

use super::helpers::*;

#[test]
fn atomic_append_replace_and_failures_keep_the_old_column_and_revision() {
    let mut set = IndexSet::new(
        vec![segment(&[(1, 1, &["abc"]), (3, 3, &["old"])], &[])],
        limits(),
    )
    .unwrap();
    let initial = set.to_bytes().unwrap();
    let initial_info = set.segments().collect::<Vec<_>>();
    let delta = segment(&[(1, 0, &["changed"]), (2, 2, &["new"])], &[1])
        .to_bytes()
        .unwrap();

    let query = QueryWorkspace::new(QueryOptions::default(), limits()).unwrap();
    let workspace = set.workspace(&query, 7).unwrap();
    assert_eq!(
        set.append_bytes(delta.clone(), None).unwrap_err().code(),
        "BUSY"
    );
    assert_eq!(set.revision(), 1);
    assert_eq!(set.to_bytes().unwrap(), initial);
    drop(workspace);
    let token = set.reserve_working(4096).unwrap();
    assert_eq!(token.capacity_bytes(), 4096);
    set.append_bytes(delta, None).unwrap();
    assert_eq!(set.revision(), 2);
    assert_eq!(set.document_count(), 3);
    let infos = set.segments().collect::<Vec<_>>();
    assert_eq!(infos[0].id, initial_info[0].id);
    assert_eq!(infos[0].dead, 1);
    let active = set.to_bytes().unwrap();
    let invalid = segment(&[(2, 8, &["conflict"])], &[]).to_bytes().unwrap();
    assert!(set.append_bytes(invalid, None).is_err());
    assert_eq!(set.revision(), 2);
    assert_eq!(set.to_bytes().unwrap(), active);
    let mut corrupt = segment(&[(8, 8, &["invalid"])], &[]).to_bytes().unwrap();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(set.replace_bytes(vec![corrupt], &[None]).is_err());
    assert_eq!(set.revision(), 2);
    assert_eq!(set.to_bytes().unwrap(), active);
    let replacement = segment(&[(9, 0, &["fresh"])], &[]).to_bytes().unwrap();
    set.replace_bytes(vec![replacement.clone()], &[None])
        .unwrap();
    assert_eq!(set.revision(), 3);
    assert_eq!(set.segment_count(), 1);
    assert_eq!(set.to_bytes().unwrap(), replacement);
    let before = set.available_working_bytes().unwrap();
    drop(token);
    assert_eq!(set.available_working_bytes().unwrap(), before + 4096);
}

#[test]
fn staging_constraints_do_not_become_permanent_runtime_limits() {
    let mut set = IndexSet::new(vec![segment(&[(1, 1, &["old"])], &[])], limits()).unwrap();
    let token = set
        .reserve_working(set.available_working_bytes().unwrap() - 512 * 1024)
        .unwrap();
    set.replace_bytes(
        vec![segment(&[(2, 2, &["new"])], &[]).to_bytes().unwrap()],
        &[None],
    )
    .unwrap();
    drop(token);

    let query = QueryWorkspace::new(QueryOptions::default(), ResourceLimits::default()).unwrap();
    assert!(set.workspace(&query, 7).is_ok());
}

#[test]
fn old_plus_new_counts_are_checked_even_for_full_replacement() {
    let small = ResourceLimits {
        max_texts: 1,
        ..limits()
    };

    let mut set = IndexSet::new(vec![segment(&[(1, 1, &["old"])], &[])], small).unwrap();
    let old = set.to_bytes().unwrap();
    let replacement = segment(&[(2, 2, &["new"])], &[]).to_bytes().unwrap();
    assert_eq!(
        set.replace_bytes(vec![replacement], &[None])
            .unwrap_err()
            .code(),
        "RESOURCE_LIMIT"
    );
    assert_eq!(set.to_bytes().unwrap(), old);
    assert_eq!(set.revision(), 1);
    // A removal-only delta adds no texts and remains possible at max_texts.
    set.append_bytes(segment(&[], &[1]).to_bytes().unwrap(), None)
        .unwrap();
    assert_eq!(set.document_count(), 0);
    assert_eq!(set.revision(), 2);
}

#[test]
fn prepared_updates_can_cancel_or_reserve_dependent_resources_before_commit() {
    let mut set = IndexSet::new(vec![segment(&[(1, 1, &["old"])], &[])], limits()).unwrap();
    let bytes = set.to_bytes().unwrap();
    let available = set.available_working_bytes().unwrap();
    let delta = segment(&[(2, 2, &["new"])], &[]).to_bytes().unwrap();
    let token = {
        let candidate = set.prepare_append(delta.clone(), None).unwrap();
        assert_eq!(candidate.text_count(), 2);
        assert_eq!(candidate.revision(), 2);
        assert!(candidate.reserve_working(usize::MAX).is_err());
        candidate.reserve_working(1024).unwrap()
    };
    assert_eq!(set.revision(), 1);
    assert_eq!(set.to_bytes().unwrap(), bytes);
    assert_eq!(set.available_working_bytes().unwrap(), available - 1024);
    drop(token);
    assert_eq!(set.available_working_bytes().unwrap(), available);
    let candidate = set.prepare_append(delta, None).unwrap();
    let token = candidate.reserve_working(1024).unwrap();
    candidate.commit();
    assert_eq!(set.revision(), 2);
    assert_eq!(set.document_count(), 2);
    let before = set.available_working_bytes().unwrap();
    drop(token);
    assert_eq!(set.available_working_bytes().unwrap(), before + 1024);
}
