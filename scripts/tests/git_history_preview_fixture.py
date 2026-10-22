"""Click Graph, Commits and File History in the real Sidebar using a private host."""
from pathlib import Path
import sys

from preview_fixture import check


if __name__ == '__main__':
    binary = str(Path(sys.argv[1]).resolve())
    for mode in ['replace', 'tab', 'pane']:
        check(binary, mode, history=True)
    check(binary, 'replace', blocked=True, history=True)
