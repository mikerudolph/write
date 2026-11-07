.PHONY: verify test docs bindings mac mac-smoke run-mac android android-smoke e2e-sync run-android clean

verify: docs test mac-smoke

docs:
	sh scripts/verify-docs.sh

test:
	cargo test -p write-core -p write-render -p write-store

bindings:
	sh scripts/build-bindings.sh

mac:
	sh scripts/build-mac.sh

mac-smoke: mac
	apps/mac/.build/release/WriteSmokeTest

run-mac: mac
	open build/mac/Write.app

android:
	sh scripts/build-android.sh

android-smoke: android
	sh scripts/smoke-android.sh

e2e-sync: android
	sh scripts/e2e-sync.sh

run-android: android
	sh scripts/run-android.sh

clean:
	cargo clean
	rm -rf build apps/mac/.build apps/android/app/build apps/android/app/src/main/jniLibs
