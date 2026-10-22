"""Navigate history and commit patches in the real viewer using a private host."""
from pathlib import Path
import sys

from viewer_close_fixture import check


if __name__ == '__main__':
    check(str(Path(sys.argv[1]).resolve()), history=True)
