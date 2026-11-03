"""Structured audit logging for Roze Memory Forensics & Defensive Analysis."""

import logging
import sys
from typing import Optional

def setup_logger(name: str = "roze_forensics", level: str = "INFO", log_file: Optional[str] = None) -> logging.Logger:
    """Configures and returns a structured forensics logger."""
    logger = logging.getLogger(name)
    logger.setLevel(getattr(logging, level.upper(), logging.INFO))
    
    # Avoid duplicate handlers if already configured
    if logger.handlers:
        return logger
        
    formatter = logging.Formatter(
        "[%(asctime)s] [%(levelname)s] [%(name)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S"
    )
    
    # Console stream handler
    sh = logging.StreamHandler(sys.stderr)
    sh.setFormatter(formatter)
    logger.addHandler(sh)
    
    # File handler for audit trails
    if log_file:
        try:
            fh = logging.FileHandler(log_file, encoding="utf-8")
            fh.setFormatter(formatter)
            logger.addHandler(fh)
        except Exception as e:
            logger.warning(f"Failed to initialize file logger at {log_file}: {e}")
            
    return logger

logger = setup_logger()
