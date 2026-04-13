CC      = gcc
CFLAGS  = -Wall -Wextra -O2
TARGET  = autoprompt
SRC     = src/autoprompt.c

$(TARGET): $(SRC)
	$(CC) $(CFLAGS) -o $@ $<

clean:
	rm -f $(TARGET)

.PHONY: clean
