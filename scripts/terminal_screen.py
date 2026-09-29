"""Small ANSI screen replay for startup layout assertions (not a full terminal)."""
import re
import unicodedata


def snapshot(data, rows, columns):
    screen = [[" "] * columns for _ in range(rows)]
    row = column = 0
    wrap_pending = False

    def newline():
        nonlocal row
        row += 1
        if row >= rows:
            screen.pop(0)
            screen.append([" "] * columns)
            row = rows - 1

    for token in re.split(r"(\x1b\[[0-?]*[ -/]*[@-~])", data.decode("utf-8", errors="replace")):
        if token.startswith("\x1b["):
            action, values = token[-1], token[2:-1]
            if values.startswith("?"):
                continue
            args = [int(value or 0) for value in values.split(";")]
            if action == "H":
                row = min(rows - 1, max(0, args[0] - 1))
                column = min(columns - 1, max(0, (args[1] if len(args) > 1 else 1) - 1))
                wrap_pending = False
            elif action == "G":
                column = min(columns - 1, max(0, args[0] - 1))
                wrap_pending = False
            elif action == "A":
                row = max(0, row - (args[0] or 1))
                wrap_pending = False
            elif action == "J":
                if args[0] == 2:
                    screen = [[" "] * columns for _ in range(rows)]
                elif args[0] == 0:
                    screen[row][column:] = [" "] * (columns - column)
                    for index in range(row + 1, rows):
                        screen[index] = [" "] * columns
            continue
        for char in token:
            if char == "\r":
                column = 0
                wrap_pending = False
            elif char == "\n":
                newline()
                wrap_pending = False
            elif ord(char) >= 32 and not unicodedata.combining(char):
                width = 2 if unicodedata.east_asian_width(char) in ("W", "F") else 1
                if wrap_pending or column + width > columns:
                    newline()
                    column = 0
                screen[row][column] = char
                if width == 2 and column + 1 < columns:
                    screen[row][column + 1] = ""
                column += width
                wrap_pending = column >= columns
                if wrap_pending:
                    column = columns - 1
    return ["".join(line) for line in screen]
