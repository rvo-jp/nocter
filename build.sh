#!/bin/sh

set -e

OUT="$HOME/.nocter/bin/nocter"
BIN_DIR="$(dirname "$OUT")"
SRC_DIR="src"

mkdir -p "$BIN_DIR"

echo "Compiling all C files in $SRC_DIR..."
gcc $(find "$SRC_DIR" -type f -name '*.c' -print) -o "$OUT" -Wall -O2

echo "Build successful: $OUT"

# Determine appropriate shell rc file
SHELL_NAME=$(basename "$SHELL")
RC_FILE=""

if [ "$SHELL_NAME" = "bash" ]; then
    RC_FILE="$HOME/.bashrc"
elif [ "$SHELL_NAME" = "zsh" ]; then
    RC_FILE="$HOME/.zshrc"
else
    RC_FILE="$HOME/.profile"
fi

# Make sure the RC file exists
[ -f "$RC_FILE" ] || touch "$RC_FILE"

# Check if $BIN_DIR is already exported in RC_FILE
if ! grep -q "$BIN_DIR" "$RC_FILE"; then
    echo "Adding $BIN_DIR to PATH in $RC_FILE..."
    {
        echo ""
        echo "# Added by Nocter build script"
        echo "export PATH=\"\$PATH:$BIN_DIR\""
    } >> "$RC_FILE"
    echo "Added PATH update to $RC_FILE"
else
    echo "$BIN_DIR already exported in $RC_FILE"
fi

# Check if $BIN_DIR is in the current PATH
case ":$PATH:" in
  *":$BIN_DIR:"*)
    echo "PATH already contains $BIN_DIR"
    ;;
  *)
    echo "PATH doesn't contain $BIN_DIR yet"
    printf "Restart shell to apply changes now? [y/N]: "
    read -r answer
    if [ "$answer" = "y" ] || [ "$answer" = "Y" ]; then
        echo "Restarting shell..."
        exec "$SHELL"
    else
        echo "Please run: exec \$SHELL or restart your terminal."
    fi
    ;;
esac