//! Flat Rust tree candidate. C storage and transient attribute getters share the
//! child root; returned strings/arrays retain one affine backing-capacity charge.
//! This synchronous traversal must only execute in the supervised parser child.
use super::{Html, Node};
use libxml::{
    budgeted::ChildAllocator,
    readonly_html::{Document, Kind},
};
use ssx_owned_budget::{Budget, Charge, OwnedVec};

pub(super) fn parse(
    points: &[u32],
    budget: &Budget,
    allocator: &ChildAllocator,
) -> Result<Html, super::OperationFailure> {
    use super::OperationFailure as Failure;
    if super::encoding_declaration(points) {
        return Err(Failure::Parse(super::rows_worker::ERRORS[1]));
    }
    let doc = Document::parse(allocator, points).map_err(|e| match e {
        libxml::readonly_html::Error::Resource => Failure::Resource,
        libxml::readonly_html::Error::Empty => Failure::Parse(super::rows_worker::ERRORS[0]),
        _ => Failure::Invariant("HTML affine parser invariant"),
    })?;
    let root = doc
        .root()
        .ok_or(Failure::Parse(super::rows_worker::ERRORS[0]))?;
    // A charged stack avoids recursive Rust stack growth and requires no extra
    // C parent/raw-pointer API. Unsupported node kinds skip their descendants,
    // exactly as the previous flat projection did.
    let mut stack = OwnedVec::new(budget).map_err(|_| Failure::Resource)?;
    stack
        .try_push((root, true))
        .map_err(|_| Failure::Resource)?;
    let mut count = 0usize;
    while let Some((node, is_root)) = stack.pop() {
        if !is_root && let Some(next) = node.next_sibling() {
            stack
                .try_push((next, false))
                .map_err(|_| Failure::Resource)?;
        }
        if matches!(node.kind(), Kind::Element | Kind::Text | Kind::Cdata) {
            count = count.checked_add(1).ok_or(Failure::Resource)?;
            if let Some(child) = node.first_child() {
                stack
                    .try_push((child, false))
                    .map_err(|_| Failure::Resource)?;
            }
        }
    }
    drop(stack);
    let mut charge = budget.reserve(0).map_err(|_| Failure::Resource)?;
    let nodes = vector::<Node>(budget, &mut charge, count)?;
    let mut tree = Html {
        nodes,
        charge: Some(super::TreeCharge(charge)),
    };
    enum Event<'a, 'b> {
        Enter(libxml::readonly_html::Node<'a, 'b>, Option<usize>, bool),
        Exit(usize),
    }
    let mut stack = OwnedVec::new(budget).map_err(|_| Failure::Resource)?;
    stack
        .try_push(Event::Enter(root, None, true))
        .map_err(|_| Failure::Resource)?;
    while let Some(event) = stack.pop() {
        let Event::Enter(node, parent, is_root) = event else {
            let Event::Exit(index) = event else {
                unreachable!()
            };
            tree.nodes[index].subtree_end = tree.nodes.len();
            continue;
        };
        if !is_root && let Some(next) = node.next_sibling() {
            stack
                .try_push(Event::Enter(next, parent, false))
                .map_err(|_| Failure::Resource)?;
        }
        if !matches!(node.kind(), Kind::Element | Kind::Text | Kind::Cdata) {
            continue;
        }
        let element = node.kind() == Kind::Element;
        let charge = &mut tree
            .charge
            .as_mut()
            .expect("charged construction invariant")
            .0;
        let name = string(
            budget,
            charge,
            if element { node.name_bytes() } else { &[] },
        )?;
        let text = string(budget, charge, node.text_bytes().unwrap_or(&[]))?;
        let mut attribute_count = 0usize;
        let mut attribute = node.first_attribute();
        while let Some(current) = attribute {
            attribute_count = attribute_count.checked_add(1).ok_or(Failure::Resource)?;
            attribute = current.next_attribute();
        }
        let mut attrs = vector(budget, charge, attribute_count)?;
        attribute = node.first_attribute();
        while let Some(current) = attribute {
            let key = string(budget, charge, current.name_bytes())?;
            let value = current.value().map_err(|_| Failure::Resource)?;
            let value = string(budget, charge, value.as_bytes())?;
            attrs.push((key, value));
            attribute = current.next_attribute();
        }
        let mut child_count = 0usize;
        let mut child = node.first_child();
        while let Some(current) = child {
            if matches!(current.kind(), Kind::Element | Kind::Text | Kind::Cdata) {
                child_count = child_count.checked_add(1).ok_or(Failure::Resource)?;
            }
            child = current.next_sibling();
        }
        let children = vector(budget, charge, child_count)?;
        let index = tree.nodes.len();
        if index >= tree.nodes.capacity() {
            return Err(Failure::Invariant("HTML node count changed"));
        }
        tree.nodes.push(Node {
            name,
            attrs,
            text,
            parent,
            children,
            subtree_end: index + 1,
        });
        if let Some(parent) = parent {
            let children = &mut tree.nodes[parent].children;
            if children.len() >= children.capacity() {
                return Err(Failure::Invariant("HTML child count changed"));
            }
            children.push(index);
        }
        stack
            .try_push(Event::Exit(index))
            .map_err(|_| Failure::Resource)?;
        if let Some(child) = node.first_child() {
            stack
                .try_push(Event::Enter(child, Some(index), false))
                .map_err(|_| Failure::Resource)?;
        }
    }
    if tree.nodes.len() != count || allocator.failed() {
        return Err(Failure::Resource);
    }
    Ok(tree)
}

fn vector<T>(
    budget: &Budget,
    owner: &mut Charge,
    count: usize,
) -> Result<Vec<T>, super::OperationFailure> {
    use super::OperationFailure as Failure;
    let size = std::alloc::Layout::array::<T>(count)
        .map_err(|_| Failure::Resource)?
        .size();
    let mut charge = budget.reserve(size).map_err(|_| Failure::Resource)?;
    // Local storage drops before its charge on all fallible paths. No existing
    // live storage is resized: every final backing allocation is sized once.
    let mut data = Vec::new();
    data.try_reserve_exact(count)
        .map_err(|_| Failure::Resource)?;
    let actual = std::alloc::Layout::array::<T>(data.capacity())
        .map_err(|_| Failure::Resource)?
        .size();
    if actual > size {
        budget.fail_closed();
        return Err(Failure::Resource);
    }
    charge.reduce_to(actual).map_err(|_| Failure::Resource)?;
    owner
        .merge_from(&mut charge)
        .map_err(|_| Failure::Resource)?;
    Ok(data)
}
fn string(
    budget: &Budget,
    owner: &mut Charge,
    bytes: &[u8],
) -> Result<String, super::OperationFailure> {
    use super::OperationFailure as Failure;
    // Match String::from_utf8_lossy without allocating a temporary Cow/String.
    let mut length = 0usize;
    let mut rest = bytes;
    loop {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                length = length.checked_add(valid.len()).ok_or(Failure::Resource)?;
                break;
            }
            Err(error) => {
                length = length
                    .checked_add(error.valid_up_to())
                    .and_then(|n| n.checked_add(3))
                    .ok_or(Failure::Resource)?;
                let Some(invalid) = error.error_len() else {
                    break;
                };
                rest = &rest[error.valid_up_to() + invalid..];
            }
        }
    }
    let mut charge = budget.reserve(length).map_err(|_| Failure::Resource)?;
    let mut out = String::new();
    out.try_reserve_exact(length)
        .map_err(|_| Failure::Resource)?;
    if out.capacity() > length {
        budget.fail_closed();
        return Err(Failure::Resource);
    }
    charge
        .reduce_to(out.capacity())
        .map_err(|_| Failure::Resource)?;
    rest = bytes;
    loop {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                out.push_str(valid);
                break;
            }
            Err(error) => {
                let valid = std::str::from_utf8(&rest[..error.valid_up_to()])
                    .map_err(|_| Failure::Invariant("HTML UTF8 prefix invariant"))?;
                out.push_str(valid);
                out.push('\u{fffd}');
                let Some(invalid) = error.error_len() else {
                    break;
                };
                rest = &rest[error.valid_up_to() + invalid..];
            }
        }
    }
    owner
        .merge_from(&mut charge)
        .map_err(|_| Failure::Resource)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn charged_lossy_conversion_matches_all_byte_pairs_and_releases_capacity() {
        let budget = Budget::new(4096).unwrap();
        for first in 0..=255u8 {
            for second in 0..=255u8 {
                let bytes = [first, second];
                let mut owner = budget.reserve(0).unwrap();
                let result = string(&budget, &mut owner, &bytes).unwrap();
                assert_eq!(result, String::from_utf8_lossy(&bytes));
                assert_eq!(owner.bytes(), result.capacity());
                assert_eq!(budget.snapshot().unwrap().used, result.capacity());
                drop(result);
                drop(owner);
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
        }
        for bytes in [
            &b"x\xf0\x9f\x92"[..],
            &b"\xf0\x9f\x92\xa9\xff"[..],
            &b"\xed\xa0\x80"[..],
        ] {
            let mut owner = budget.reserve(0).unwrap();
            let result = string(&budget, &mut owner, bytes).unwrap();
            assert_eq!(result, String::from_utf8_lossy(bytes));
            drop(result);
            drop(owner);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
    #[test]
    fn flat_backing_refusal_retains_existing_storage_until_owner_drop() {
        let budget = Budget::new(128).unwrap();
        let mut owner = budget.reserve(0).unwrap();
        let mut values = vector::<u64>(&budget, &mut owner, 8).unwrap();
        values.extend(0..8);
        let text = string(&budget, &mut owner, b"owned tree bytes").unwrap();
        let used = budget.snapshot().unwrap().used;
        assert_eq!(used, values.capacity() * 8 + text.capacity());
        assert!(matches!(
            vector::<u64>(&budget, &mut owner, 17),
            Err(super::super::OperationFailure::Resource)
        ));
        assert!(matches!(
            string(&budget, &mut owner, &[b'x'; 129]),
            Err(super::super::OperationFailure::Resource)
        ));
        assert_eq!(values, (0..8).collect::<Vec<_>>());
        assert_eq!(budget.snapshot().unwrap().used, used);
        drop(values);
        drop(text);
        drop(owner);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
