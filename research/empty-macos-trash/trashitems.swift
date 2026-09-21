// Trashes each path with FileManager.trashItem, the call the `trash` crate's NsFileManager
// method makes, then stays alive 3 seconds so the Put Back records are written.
// Usage: trashitems <path>...   Prints "<original>\t<resulting path>" or the full error per item.
import Foundation

for path in CommandLine.arguments.dropFirst() {
    var resulting: NSURL?
    do {
        try FileManager.default.trashItem(at: URL(fileURLWithPath: path), resultingItemURL: &resulting)
        print("\(path)\tOK\t\(resulting?.path ?? "")")
    } catch let error as NSError {
        print("\(path)\tERROR\t\(error.domain) \(error.code)\t\(error.localizedDescription)")
    }
}
Thread.sleep(forTimeInterval: 3)
