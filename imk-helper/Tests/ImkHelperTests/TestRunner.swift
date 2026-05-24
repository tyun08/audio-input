// Bare-bones test runner. Each test is a () throws -> Void closure.
// Caller invokes run([...]); we print one line per test, then a summary
// and exit non-zero if any test threw. Replaces XCTest because Apple's
// Command Line Tools (no Xcode) doesn't ship XCTest.

import Foundation

struct TestCase {
    let name: String
    let run: () throws -> Void
}

enum TestError: Error, CustomStringConvertible {
    case failed(String)
    var description: String {
        if case .failed(let m) = self { return m } else { return "" }
    }
}

func assertEq<T: Equatable>(_ a: T, _ b: T, _ msg: String = "", file: String = #file, line: Int = #line) throws {
    if a != b {
        throw TestError.failed("assertEq failed\n  left:  \(a)\n  right: \(b)\n  \(msg)\n  at \(file):\(line)")
    }
}
func assertTrue(_ cond: Bool, _ msg: String = "", file: String = #file, line: Int = #line) throws {
    if !cond { throw TestError.failed("assertTrue failed: \(msg) at \(file):\(line)") }
}
func assertFalse(_ cond: Bool, _ msg: String = "", file: String = #file, line: Int = #line) throws {
    if cond { throw TestError.failed("assertFalse failed: \(msg) at \(file):\(line)") }
}
func assertThrows<T>(_ expr: @autoclosure () throws -> T, file: String = #file, line: Int = #line) throws {
    do {
        _ = try expr()
        throw TestError.failed("assertThrows: expression did not throw at \(file):\(line)")
    } catch is TestError {
        throw TestError.failed("assertThrows: expression did not throw at \(file):\(line)")
    } catch {
        // Any non-TestError is the throw we wanted.
    }
}

func runAll(_ cases: [TestCase]) -> Never {
    var passed = 0
    var failures: [(String, Error)] = []
    for tc in cases {
        do {
            try tc.run()
            passed += 1
            print("  ✓ \(tc.name)")
        } catch {
            failures.append((tc.name, error))
            print("  ✗ \(tc.name)")
        }
    }
    print("")
    print("\(passed)/\(cases.count) passed")
    if !failures.isEmpty {
        print("")
        print("Failures:")
        for (name, err) in failures {
            print("  • \(name)")
            print("    \(err)")
        }
        exit(1)
    }
    exit(0)
}
