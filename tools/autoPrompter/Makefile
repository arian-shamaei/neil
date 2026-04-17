CC      = gcc
CFLAGS  = -Wall -Wextra -O2
TARGET  = autoprompt
SRC     = src/autoprompt.c

HEADER_SRC = src/parse_prompt_header.c
HEADER_HDR = src/parse_prompt_header.h
TEST_SRC   = src/test_parse_prompt_header.c
TEST_BIN   = test_parse_prompt_header

$(TARGET): $(SRC)
	$(CC) $(CFLAGS) -o $@ $<

# Standalone parser library (ready to link into autoprompt.c).
parse_prompt_header.o: $(HEADER_SRC) $(HEADER_HDR)
	$(CC) $(CFLAGS) -c $(HEADER_SRC) -o $@

$(TEST_BIN): $(TEST_SRC) parse_prompt_header.o
	$(CC) $(CFLAGS) -o $@ $(TEST_SRC) parse_prompt_header.o

test-parser: $(TEST_BIN)
	./$(TEST_BIN)

clean:
	rm -f $(TARGET) $(TEST_BIN) parse_prompt_header.o

.PHONY: clean test-parser
