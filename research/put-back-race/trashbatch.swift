// Trashes each path in turn with FileManager.trashItem, the call the `trash` crate's
// NsFileManager method makes, sleeping <pause> seconds between calls,
// then staying alive <linger> seconds, as a long-lived program would.
// Usage: trashbatch <pause seconds> <linger seconds> <path>...   Prints "<original>\t<name in the Trash>" per item.
import Foundation

let args = CommandLine.arguments
guard args.count >= 4, let pause = Double(args[1]), let linger = Double(args[2]) else {
    FileHandle.standardError.write("usage: trashbatch <pause> <linger> <path>...\n".data(using: .utf8)!)
    exit(2)
}

let paths = Array(args[3...])
for (index, path) in paths.enumerated() {
    var resulting: NSURL?
    do {
        try FileManager.default.trashItem(at: URL(fileURLWithPath: path), resultingItemURL: &resulting)
        print("\(path)\t\(resulting?.lastPathComponent ?? "")")
    } catch {
        print("\(path)\tERROR \(error.localizedDescription)")
    }
    if pause > 0 && index < paths.count - 1 {
        Thread.sleep(forTimeInterval: pause)
    }
}
if linger > 0 {
    Thread.sleep(forTimeInterval: linger)
}
