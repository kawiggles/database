package main

import (
	"bufio"
	"fmt"
	"net"
	"os"
	"strings"
)

func main() {
	addr := "localhost:55555"
	conn, err := net.Dial("tcp", addr)

	if err != nil {
		fmt.Printf("Error connecting to database: %v\n", err)
		os.Exit(1)
	}

	defer conn.Close()

	message := getMessage()
	payload := []byte(message)

	bytesWritten, err := conn.Write(payload)
	if err != nil {
		fmt.Printf("Error writing to socket: %v\n", err)
	}
}

func getMessage() string {
	scanner := bufio.NewScanner(os.Stdin)
	fmt.Print("Database prompt: ")
	
	if scanner.Scan() {
		input := strings.Fields(scanner.Text())
		switch input {
		case "GET":
		case "SET":
		case "DEL":
		case "HELP":
		case "EXIT":
		default:
		}
	}

}

