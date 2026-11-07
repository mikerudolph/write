import Darwin
import Foundation

func now() -> UInt64 { DispatchTime.now().uptimeNanoseconds }
func milliseconds(_ start: UInt64, _ end: UInt64) -> Double { Double(end - start) / 1_000_000 }

func percentile(_ values: [Double], _ fraction: Double) -> Double {
    guard !values.isEmpty else { return 0 }
    let sorted = values.sorted()
    return sorted[Int(Double(sorted.count - 1) * fraction)]
}

func summary(_ values: [Double]) -> [String: Double] {
    [
        "count": Double(values.count),
        "p50": percentile(values, 0.5),
        "p95": percentile(values, 0.95),
        "p99": percentile(values, 0.99),
        "max": values.max() ?? 0,
    ]
}

func footprintBytes(pid: pid_t) -> UInt64 {
    var info = rusage_info_v4()
    let result = withUnsafeMutablePointer(to: &info) { pointer in
        pointer.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) {
            proc_pid_rusage(pid, RUSAGE_INFO_V4, $0)
        }
    }
    return result == 0 ? info.ri_phys_footprint : 0
}

func webKitProcessIDs() -> Set<pid_t> {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/ps")
    process.arguments = ["-axo", "pid=,comm="]
    let pipe = Pipe()
    process.standardOutput = pipe
    try? process.run()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    var result = Set<pid_t>()
    for line in String(decoding: data, as: UTF8.self).split(separator: "\n") where line.contains("com.apple.WebKit.") {
        if let value = line.split(separator: " ").first, let pid = pid_t(value) {
            result.insert(pid)
        }
    }
    return result
}
