package main

import (
	"bufio"
	"context"
	"fmt"
	"github.com/Ahcast/Bastion/backend/database"
	"github.com/Ahcast/Bastion/backend/internal/core/runtime"
	"golang.org/x/term"
	"os"
	"strings"
	"time"
)

func main() {
	if len(os.Args) != 2 || (os.Args[1] != "migrate" && os.Args[1] != "bootstrap") {
		fmt.Fprintln(os.Stderr, "Usage: admin migrate|bootstrap")
		os.Exit(2)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	defer cancel()
	s, _, e := runtime.Open(ctx)
	if e != nil {
		fmt.Fprintln(os.Stderr, "Configuration or database unavailable")
		os.Exit(1)
	}
	defer s.DB.Close()
	if os.Args[1] == "migrate" {
		e = database.Migrate(ctx, s.DB)
	} else {
		if !term.IsTerminal(int(os.Stdin.Fd())) {
			fmt.Fprintln(os.Stderr, "Bootstrap requires a terminal; password is not accepted in arguments/env")
			os.Exit(2)
		}
		in := bufio.NewReader(os.Stdin)
		fmt.Print("Correo: ")
		email, _ := in.ReadString('\n')
		fmt.Print("Nombre: ")
		name, _ := in.ReadString('\n')
		fmt.Print("Contraseña (15+ caracteres): ")
		password, readErr := term.ReadPassword(int(os.Stdin.Fd()))
		fmt.Println()
		if readErr != nil {
			e = readErr
		} else {
			e = s.Bootstrap(ctx, strings.TrimSpace(email), strings.TrimSpace(name), string(password))
		}
		clear(password)
	}
	if e != nil {
		fmt.Fprintln(os.Stderr, "Operation failed:", e)
		os.Exit(1)
	}
	fmt.Println("Operation completed")
}
