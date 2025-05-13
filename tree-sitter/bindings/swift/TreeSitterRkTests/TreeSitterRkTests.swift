import XCTest
import SwiftTreeSitter
import TreeSitterRk

final class TreeSitterRkTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_rk())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading Rk grammar")
    }
}
