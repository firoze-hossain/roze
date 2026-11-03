"""Shannon entropy calculation and block-level entropy distribution analysis."""

import math
from typing import List, Tuple, Dict

def calculate_shannon_entropy(data: bytes) -> float:
    """Calculates the Shannon entropy of a byte buffer (0.0 to 8.0 bits)."""
    if not data:
        return 0.0
        
    length = len(data)
    frequencies = [0] * 256
    for b in data:
        frequencies[b] += 1
        
    entropy = 0.0
    for count in frequencies:
        if count > 0:
            p = count / length
            entropy -= p * math.log2(p)
            
    return entropy

def analyze_entropy_blocks(data: bytes, block_size: int = 1024) -> List[Tuple[int, float]]:
    """Divides data into blocks and computes entropy per block.
    
    Returns a list of (offset, entropy) tuples.
    """
    if not data or block_size <= 0:
        return []
        
    results = []
    total_len = len(data)
    for offset in range(0, total_len, block_size):
        chunk = data[offset : offset + block_size]
        entropy = calculate_shannon_entropy(chunk)
        results.append((offset, entropy))
        
    return results

def is_high_entropy(data: bytes, threshold: float = 7.2) -> bool:
    """Returns True if the buffer exceeds the given Shannon entropy threshold."""
    return calculate_shannon_entropy(data) >= threshold
