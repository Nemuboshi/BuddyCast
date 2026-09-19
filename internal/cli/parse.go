package cli

import (
	"errors"
	"flag"
	"fmt"
	"strconv"
	"strings"
)

const usage = `Usage:
  buddycast getinfo [--timeout SEC] [--save PATH] [--limit COUNT|all]
  buddycast fetch [--timeout SEC] [--out DIR] [--keep-encrypted-zip] [--srt] [--offline] <asset_id>

Commands:
  getinfo  Fetch and print the available subtitle packages
  fetch    Download, decrypt, extract, and convert one package
`

type HelpError struct{}

func (HelpError) Error() string { return usage }

type Command struct {
	Name, AssetID, Save, Out       string
	Timeout                        float64
	Limit                          int
	KeepEncryptedZip, SRT, Offline bool
}

func Parse(args []string) (Command, error) {
	if len(args) == 0 || args[0] == "help" || args[0] == "-h" || args[0] == "--help" {
		return Command{}, HelpError{}
	}
	switch args[0] {
	case "getinfo":
		return parseGetinfo(args[1:])
	case "fetch":
		return parseFetch(args[1:])
	default:
		return Command{}, fmt.Errorf("unknown command %q\n\n%s", args[0], usage)
	}
}

func parseGetinfo(args []string) (Command, error) {
	fs := flag.NewFlagSet("getinfo", flag.ContinueOnError)
	cmd := Command{Name: "getinfo", Timeout: 60, Limit: 50}
	var limit string
	fs.Float64Var(&cmd.Timeout, "timeout", 60, "HTTP timeout in seconds")
	fs.StringVar(&cmd.Save, "save", "", "save contents JSON")
	fs.StringVar(&limit, "limit", "50", "row count or all")
	if err := fs.Parse(hoist(args, "timeout", "save", "limit")); err != nil {
		return Command{}, err
	}
	if len(fs.Args()) != 0 {
		return Command{}, errors.New("getinfo takes no arguments")
	}
	if limit == "all" {
		cmd.Limit = 0
	} else {
		n, err := strconv.Atoi(limit)
		if err != nil || n < 1 {
			return Command{}, errors.New("limit must be a positive integer or all")
		}
		cmd.Limit = n
	}
	return cmd, nil
}

func parseFetch(args []string) (Command, error) {
	fs := flag.NewFlagSet("fetch", flag.ContinueOnError)
	cmd := Command{Name: "fetch", Timeout: 60, Out: "downloads"}
	fs.Float64Var(&cmd.Timeout, "timeout", 60, "HTTP timeout in seconds")
	fs.StringVar(&cmd.Out, "out", "downloads", "output directory")
	fs.BoolVar(&cmd.KeepEncryptedZip, "keep-encrypted-zip", false, "keep encrypted package")
	fs.BoolVar(&cmd.SRT, "srt", false, "also create SRT subtitles")
	fs.BoolVar(&cmd.Offline, "offline", false, "use files from downloads")
	if err := fs.Parse(hoist(args, "timeout", "out")); err != nil {
		return Command{}, err
	}
	if len(fs.Args()) != 1 {
		return Command{}, errors.New("fetch requires exactly one asset_id")
	}
	cmd.AssetID = fs.Args()[0]
	return cmd, nil
}

// hoist moves flags before positional args so flag.Parse accepts any order.
func hoist(args []string, valueFlags ...string) []string {
	takesValue := make(map[string]bool, len(valueFlags))
	for _, name := range valueFlags {
		takesValue[name] = true
	}
	var flags, positional []string
	for i := 0; i < len(args); i++ {
		arg := args[i]
		name := strings.TrimPrefix(strings.TrimPrefix(strings.SplitN(arg, "=", 2)[0], "--"), "-")
		if !strings.HasPrefix(arg, "-") {
			positional = append(positional, arg)
			continue
		}
		flags = append(flags, arg)
		if takesValue[name] && !strings.Contains(arg, "=") && i+1 < len(args) {
			i++
			flags = append(flags, args[i])
		}
	}
	return append(flags, positional...)
}
