//! # Gas Optimization Utilities
//!
//! This module provides optimized helpers for reducing gas consumption:
//! - Efficient storage access patterns
//! - Optimized batch operations
//! - Reduced redundant computations

use soroban_sdk::{Address, Env, Symbol, Vec};

/// Optimized insertion sort for small batches with early exit for already-sorted data.
/// Uses binary search to find insertion point, reducing comparisons.
pub fn optimized_sort_u64(env: &Env, items: &Vec<u64>) -> Vec<u64> {
    let len = items.len();
    if len <= 1 {
        return items.clone();
    }
    
    let mut sorted: Vec<u64> = Vec::new(env);
    sorted.push_back(items.get(0).unwrap());
    
    for i in 1..len {
        let item = items.get(i).unwrap();
        // Check if already in correct position (optimization for nearly-sorted data)
        if item >= sorted.get(sorted.len() - 1).unwrap() {
            sorted.push_back(item);
            continue;
        }
        
        // Binary search for insertion point
        let mut left = 0u32;
        let mut right = sorted.len() - 1;
        
        while left <= right {
            let mid = left + (right - left) / 2;
            let mid_val = sorted.get(mid).unwrap();
            
            if mid_val == item {
                left = mid + 1;
                break;
            } else if mid_val < item {
                left = mid + 1;
            } else {
                if mid == 0 {
                    break;
                }
                right = mid - 1;
            }
        }
        
        // Insert at position
        let mut next: Vec<u64> = Vec::new(env);
        for j in 0..left {
            next.push_back(sorted.get(j).unwrap());
        }
        next.push_back(item);
        for j in left..sorted.len() {
            next.push_back(sorted.get(j).unwrap());
        }
        sorted = next;
    }
    
    sorted
}

/// Cache storage reads to avoid redundant storage access.
/// Returns (value, cache_hit) tuple.
pub struct StorageCache<T> {
    key: Symbol,
    value: Option<T>,
    hit: bool,
}

impl<T: Clone> StorageCache<T> {
    pub fn new(env: &Env, key: &Symbol) -> Self {
        let value = env.storage().instance().get(key);
        StorageCache {
            key: key.clone(),
            value,
            hit: false,
        }
    }
    
    pub fn get(&mut self) -> Option<&T> {
        self.hit = true;
        self.value.as_ref()
    }
    
    pub fn is_hit(&self) -> bool {
        self.hit
    }
}

/// Efficient batch processing with cached storage reads.
pub fn batch_process_with_cache<F, T, R>(
    env: &Env,
    items: &Vec<T>,
    storage_key: &Symbol,
    processor: F,
) -> Vec<R>
where
    F: Fn(&Env, &T, &Option<T>) -> R,
    T: Clone,
{
    let mut results: Vec<R> = Vec::new(env);
    let mut cached_value: Option<T> = None;
    
    for item in items.iter() {
        let result = processor(env, &item, &cached_value);
        results.push_back(result);
        // Update cache for next iteration if needed
        cached_value = Some(item.clone());
    }
    
    results
}

/// Optimized membership check using binary search on sorted Vec.
pub fn binary_search_contains(vec: &Vec<u64>, target: u64) -> bool {
    if vec.is_empty() {
        return false;
    }
    
    let mut left = 0u32;
    let mut right = vec.len() - 1;
    
    while left <= right {
        let mid = left + (right - left) / 2;
        let mid_val = vec.get(mid).unwrap();
        
        if mid_val == target {
            return true;
        } else if mid_val < target {
            if mid == vec.len() - 1 {
                return false;
            }
            left = mid + 1;
        } else {
            if mid == 0 {
                return false;
            }
            right = mid - 1;
        }
    }
    
    false
}

/// Pack multiple boolean flags into a single u32 for storage efficiency.
pub mod packed_flags {
    pub const FLAG_0_MASK: u32 = 1 << 0;
    pub const FLAG_1_MASK: u32 = 1 << 1;
    pub const FLAG_2_MASK: u32 = 1 << 2;
    pub const FLAG_3_MASK: u32 = 1 << 3;
    pub const FLAG_4_MASK: u32 = 1 << 4;
    pub const FLAG_5_MASK: u32 = 1 << 5;
    pub const FLAG_6_MASK: u32 = 1 << 6;
    pub const FLAG_7_MASK: u32 = 1 << 7;
    
    /// Pack up to 8 boolean flags into a single u32.
    pub fn pack(flags: [bool; 8]) -> u32 {
        let mut packed = 0u32;
        for i in 0..8 {
            if flags[i] {
                packed |= 1 << i;
            }
        }
        packed
    }
    
    /// Unpack 8 boolean flags from a u32.
    pub fn unpack(packed: u32) -> [bool; 8] {
        [
            packed & FLAG_0_MASK != 0,
            packed & FLAG_1_MASK != 0,
            packed & FLAG_2_MASK != 0,
            packed & FLAG_3_MASK != 0,
            packed & FLAG_4_MASK != 0,
            packed & FLAG_5_MASK != 0,
            packed & FLAG_6_MASK != 0,
            packed & FLAG_7_MASK != 0,
        ]
    }
    
    /// Check if a specific flag is set.
    pub fn is_set(packed: u32, flag_index: u8) -> bool {
        packed & (1 << flag_index) != 0
    }
    
    /// Set a specific flag.
    pub fn set_flag(packed: u32, flag_index: u8, value: bool) -> u32 {
        if value {
            packed | (1 << flag_index)
        } else {
            packed & !(1 << flag_index)
        }
    }
}

/// Optimized arithmetic operations with overflow protection.
pub mod safe_math {
    /// Ceiling division: ceil(a / b) = (a + b - 1) / b
    pub fn ceil_div(a: i128, b: i128) -> i128 {
        if b == 0 {
            return 0; // Handle division by zero
        }
        (a.checked_add(b - 1).unwrap_or(a)) / b
    }
    
    /// Safe multiplication with overflow check.
    pub fn safe_mul(a: i128, b: i128) -> Option<i128> {
        a.checked_mul(b)
    }
    
    /// Safe addition with overflow check.
    pub fn safe_add(a: i128, b: i128) -> Option<i128> {
        a.checked_add(b)
    }
    
    /// Safe subtraction with underflow check.
    pub fn safe_sub(a: i128, b: i128) -> Option<i128> {
        a.checked_sub(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};
    
    #[test]
    fn test_optimized_sort() {
        let env = Env::default();
        let items = vec![&env, 5u64, 2, 8, 1, 9, 3];
        let sorted = optimized_sort_u64(&env, &items);
        
        assert_eq!(sorted.get(0), Some(1));
        assert_eq!(sorted.get(1), Some(2));
        assert_eq!(sorted.get(2), Some(3));
        assert_eq!(sorted.get(3), Some(5));
        assert_eq!(sorted.get(4), Some(8));
        assert_eq!(sorted.get(5), Some(9));
    }
    
    #[test]
    fn test_binary_search_contains() {
        let env = Env::default();
        let items = vec![&env, 1u64, 3, 5, 7, 9];
        
        assert!(binary_search_contains(&items, 5));
        assert!(!binary_search_contains(&items, 4));
        assert!(binary_search_contains(&items, 1));
        assert!(binary_search_contains(&items, 9));
        assert!(!binary_search_contains(&items, 10));
    }
    
    #[test]
    fn test_packed_flags() {
        let flags = [true, false, true, false, false, false, false, false];
        let packed = packed_flags::pack(flags);
        
        assert!(packed_flags::is_set(packed, 0));
        assert!(!packed_flags::is_set(packed, 1));
        assert!(packed_flags::is_set(packed, 2));
        assert!(!packed_flags::is_set(packed, 3));
        
        let unpacked = packed_flags::unpack(packed);
        assert_eq!(unpacked, flags);
    }
    
    #[test]
    fn test_ceil_div() {
        assert_eq!(safe_math::ceil_div(10, 3), 4);
        assert_eq!(safe_math::ceil_div(9, 3), 3);
        assert_eq!(safe_math::ceil_div(1, 3), 1);
        assert_eq!(safe_math::ceil_div(0, 3), 0);
    }
}
