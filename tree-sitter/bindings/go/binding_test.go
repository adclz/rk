package tree_sitter_rk_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_rk "github.com/adclz/rk/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_rk.Language())
	if language == nil {
		t.Errorf("Error loading Rk grammar")
	}
}
