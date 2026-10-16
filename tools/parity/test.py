# /// script
# requires-python = ">=3.11"
# dependencies = ["python-pptx>=1.0", "scikit-image>=0.24", "pillow>=10", "numpy>=1.26", "pypdf==6.20.1"]
# ///
"""Run isolated parity contracts with fake executables, not installed rendering apps."""

from pathlib import Path
import unittest

suite = unittest.defaultTestLoader.discover(str(Path(__file__).parent / "tests"))
raise SystemExit(not unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful())
