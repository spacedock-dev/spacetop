#!/usr/bin/env python3
import json, os, sys
from pathlib import Path

Path(os.environ["TEST_HERDR_ROOT"], "spacetop.json").write_text(
    json.dumps({"argv": sys.argv[1:], "cwd": os.getcwd()})
)
