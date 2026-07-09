# anetd API → WASM (Emscripten)
#
# CI prereq (Ubuntu):   apt install emscripten

EMCC   ?= emcc
CFLAGS = -O3 -Wall -Wextra
EMFLAGS = \
	-s WASM=1 \
	-s SINGLE_FILE=1 \
	-s EXPORT_ES6=1 \
	-s ENVIRONMENT=web \
	-s EXPORTED_FUNCTIONS='["_get_status","_load_rules","_reload_rules","_toggle_filter","_load_config","_save_config","_load_logs","_check_url","_malloc","_free"]' \
	-s EXPORTED_RUNTIME_METHODS='["ccall","cwrap","UTF8ToString","stringToUTF8","lengthBytesUTF8"]' \
	-s ALLOW_MEMORY_GROWTH=1 \
	-s MODULARIZE=1 \
	-s EXPORT_NAME='AnetdWasm'

SRC   = anetd_api.c
OUT   = ../api/anetd_wasm_core.js

.PHONY: all clean

all: $(OUT)

$(OUT): $(SRC)
	@mkdir -p $(@D)
	$(EMCC) $(CFLAGS) $(EMFLAGS) -o $@ $<

clean:
	rm -f $(OUT)
