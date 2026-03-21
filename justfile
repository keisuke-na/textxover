rust_target := "aarch64-apple-darwin"
swift_sources := "swift/main.swift swift/AppDelegate.swift swift/OverlayWindow.swift swift/TextRasterizer.swift swift/DisplaySelector.swift"

# Rust dylib build
build-rust:
    cd rust && cargo build --release

# Swift build + link
build-swift: build-rust
    swiftc -O \
        -import-objc-header include/textxover.h \
        -L rust/target/release \
        -ltextxover \
        -o textxover \
        {{swift_sources}}

# Create app bundle
bundle: build-swift
    mkdir -p textxover.app/Contents/MacOS
    mkdir -p textxover.app/Contents/Resources
    cp textxover textxover.app/Contents/MacOS/
    cp rust/target/release/libtextxover.dylib textxover.app/Contents/MacOS/
    cp resources/Info.plist textxover.app/Contents/
    cp resources/AppIcon.icns textxover.app/Contents/Resources/
    install_name_tool -change \
        libtextxover.dylib \
        @executable_path/libtextxover.dylib \
        textxover.app/Contents/MacOS/textxover

# Build + run (kills existing process first)
run: bundle
    -pkill -f textxover.app/Contents/MacOS/textxover
    sleep 1
    open textxover.app

# Test: send comment
test-comment:
    curl -X POST http://localhost:2525/comment \
        -H "Content-Type: application/json" \
        -d '{"text":"テスト！","color":"#FF0000","size":"big"}'

# Test: firework
test-firework:
    curl -X POST http://localhost:2525/effect \
        -H "Content-Type: application/json" \
        -d '{"type":"firework","x":0.5,"y":0.5}'

# Test: status
test-status:
    curl http://localhost:2525/status

clean:
    cd rust && cargo clean
    rm -rf textxover textxover.app
