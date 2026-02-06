#![no_std]

use core::cmp::max;
use core::mem::size_of;
use core::ptr::{null_mut};

//
// Type aliases (match your C headers)
//

pub type AvlKey = usize;
pub type AvlVal = usize;

//
// External kernel helpers
//

extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn calloc(size: usize, count: usize) -> *mut u8;
    fn free(ptr: *mut u8);

    fn panic() -> !;
}

macro_rules! assert {
    ($cond:expr) => {
        if !$cond {
            unsafe { panic() }
        }
    };
}

//
// AVL node
//

#[repr(C)]
pub struct AvlHeader {
    pub key: AvlKey,
    pub value: AvlVal,
    pub height: i32,
    pub left: *mut AvlHeader,
    pub right: *mut AvlHeader,
}

//
// Helpers (macro equivalents)
//

#[inline]
unsafe fn calc_height(node: *mut AvlHeader) -> i32 {
    if node.is_null() {
        0
    } else {
        (*node).height
    }
}

#[inline]
unsafe fn calc_balance(node: *mut AvlHeader) -> i32 {
    if node.is_null() {
        0
    } else {
        calc_height((*node).left) - calc_height((*node).right)
    }
}

//
// Allocation
//

pub unsafe fn avl_allocate_node(key: AvlKey) -> *mut AvlHeader {
    let node = calloc(size_of::<AvlHeader>(), 1) as *mut AvlHeader;
    assert!(!node.is_null());

    (*node).key = key;
    (*node).height = 1;

    node
}

//
// Debug (optional, mirrors your C version)
//

#[allow(dead_code)]
pub unsafe fn avl_debug(root: *mut AvlHeader, space: i32) {
    const COUNT: i32 = 10;

    if root.is_null() {
        return;
    }

    let space = space + COUNT;

    avl_debug((*root).right, space);

    // printing omitted intentionally (kernel-side)
    // replace with debugf if desired

    avl_debug((*root).left, space);
}

//
// Rotations
//

unsafe fn avl_rotate_right(y: *mut AvlHeader) -> *mut AvlHeader {
    let x = (*y).left;
    let t2 = (*x).right;

    (*x).right = y;
    (*y).left = t2;

    (*y).height = max(
        calc_height((*y).left),
        calc_height((*y).right),
    ) + 1;

    (*x).height = max(
        calc_height((*x).left),
        calc_height((*x).right),
    ) + 1;

    x
}

unsafe fn avl_rotate_left(x: *mut AvlHeader) -> *mut AvlHeader {
    let y = (*x).right;
    let t2 = (*y).left;

    (*y).left = x;
    (*x).right = t2;

    (*x).height = max(
        calc_height((*x).left),
        calc_height((*x).right),
    ) + 1;

    (*y).height = max(
        calc_height((*y).left),
        calc_height((*y).right),
    ) + 1;

    y
}

//
// Min node
//

unsafe fn avl_min_key_node(start: *mut AvlHeader) -> *mut AvlHeader {
    let mut browse = start;
    while !browse.is_null() && !(*browse).left.is_null() {
        browse = (*browse).left;
    }
    browse
}

//
// Insert (recursive)
//

unsafe fn avl_allocate_l(
    node: *mut AvlHeader,
    key: AvlKey,
    target: &mut *mut AvlHeader,
) -> *mut AvlHeader {
    if node.is_null() {
        let actual = avl_allocate_node(key);
        assert!(target.is_null());
        *target = actual;
        return actual;
    }

    if key < (*node).key {
        (*node).left = avl_allocate_l((*node).left, key, target);
    } else if key > (*node).key {
        (*node).right = avl_allocate_l((*node).right, key, target);
    } else {
        // duplicate key
        unsafe { panic() }
    }

    (*node).height = 1 + max(
        calc_height((*node).left),
        calc_height((*node).right),
    );

    let balance = calc_balance(node);

    // Left Left
    if balance > 1 && key < (*(*node).left).key {
        return avl_rotate_right(node);
    }

    // Right Right
    if balance < -1 && key > (*(*node).right).key {
        return avl_rotate_left(node);
    }

    // Left Right
    if balance > 1 && key > (*(*node).left).key {
        (*node).left = avl_rotate_left((*node).left);
        return avl_rotate_right(node);
    }

    // Right Left
    if balance < -1 && key < (*(*node).right).key {
        (*node).right = avl_rotate_right((*node).right);
        return avl_rotate_left(node);
    }

    node
}

//
// Delete (recursive)
//

unsafe fn avl_unregister_l(
    root: *mut AvlHeader,
    key: AvlKey,
    target: &mut AvlVal,
) -> *mut AvlHeader {
    if root.is_null() {
        return root;
    }

    if key < (*root).key {
        (*root).left = avl_unregister_l((*root).left, key, target);
    } else if key > (*root).key {
        (*root).right = avl_unregister_l((*root).right, key, target);
    } else {
        if (*root).left.is_null() || (*root).right.is_null() {
            let temp = if !(*root).left.is_null() {
                (*root).left
            } else {
                (*root).right
            };

            if temp.is_null() {
                *target = (*root).value;
                free(root as *mut u8);
                return null_mut();
            } else {
                *target = (*temp).value;
                core::ptr::copy_nonoverlapping(temp, root, 1);
                free(temp as *mut u8);
            }
        } else {
            let temp = avl_min_key_node((*root).right);
            (*root).key = (*temp).key;
            (*root).value = (*temp).value;
            (*root).right = avl_unregister_l((*root).right, (*temp).key, target);
        }
    }

    if root.is_null() {
        return root;
    }

    (*root).height = 1 + max(
        calc_height((*root).left),
        calc_height((*root).right),
    );

    let balance = calc_balance(root);

    // Left Left
    if balance > 1 && calc_balance((*root).left) >= 0 {
        return avl_rotate_right(root);
    }

    // Left Right
    if balance > 1 && calc_balance((*root).left) < 0 {
        (*root).left = avl_rotate_left((*root).left);
        return avl_rotate_right(root);
    }

    // Right Right
    if balance < -1 && calc_balance((*root).right) <= 0 {
        return avl_rotate_left(root);
    }

    // Right Left
    if balance < -1 && calc_balance((*root).right) > 0 {
        (*root).right = avl_rotate_right((*root).right);
        return avl_rotate_left(root);
    }

    root
}

//
// Public API
//

pub unsafe fn avl_allocate(
    root_ptr: &mut *mut AvlHeader,
    key: AvlKey,
    value: AvlVal,
) -> *mut AvlHeader {
    let mut target: *mut AvlHeader = null_mut();
    *root_ptr = avl_allocate_l(*root_ptr, key, &mut target);
    assert!(!target.is_null());
    (*target).value = value;
    target
}

pub unsafe fn avl_unregister(root_ptr: &mut *mut AvlHeader, key: AvlKey) -> bool {
    let mut target: AvlVal = 0;
    *root_ptr = avl_unregister_l(*root_ptr, key, &mut target);
    target != 0
}

pub unsafe fn avl_lookup(root: *mut AvlHeader, key: AvlKey) -> AvlVal {
    if root.is_null() {
        return 0;
    }

    if key > (*root).key {
        avl_lookup((*root).right, key)
    } else if key < (*root).key {
        avl_lookup((*root).left, key)
    } else {
        (*root).value
    }
}
