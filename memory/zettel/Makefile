CC      = gcc
CFLAGS  = -Wall -Wextra -O2
TARGET  = zettel
SRC     = src/zettel.c

$(TARGET): $(SRC)
	$(CC) $(CFLAGS) -o $@ $<

clean:
	rm -f $(TARGET)

.PHONY: clean
