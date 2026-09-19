package main

import (
	"context"
	"errors"
	"fmt"
	"os"

	"github.com/Nemuboshi/buddycast/internal/app"
	"github.com/Nemuboshi/buddycast/internal/cli"
)

func main() {
	cmd, err := cli.Parse(os.Args[1:])
	if err != nil {
		var help cli.HelpError
		if errors.As(err, &help) {
			fmt.Fprintln(os.Stdout, err)
			return
		}
		fmt.Fprintln(os.Stderr, err)
		os.Exit(2)
	}
	if err := app.Run(context.Background(), cmd); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}
