deps:
	# The tag needs to be compatible with `cairo-lang-*` dependencies.
	git clone --depth=1 -b v2.8.2 https://github.com/starkware-libs/cairo.git \
	&& mv cairo/corelib/ . \
	&& rm -rf cairo/

clean:
	rm -rf corelib
	rm -rf cairo
