package main

import (
	"bufio"
	"fmt"
	"net"
	"os"
)

func main() {
	addr := "localhost:55555"
	conn, err := net.Dial("tcp", addr)
	if err != nil {
		fmt.Printf("Error connecting to database: %v\n", err)
		os.Exit(1)
	}
	defer conn.Close()

	scanner := bufio.NewScanner(os.Stdin)
	for {
		fmt.Print("Database prompt: ")

		if scanner.Scan() {
			input := scanner.Text()
			if input == "EXIT" {
				break;
			}

			_, err := conn.Write([]byte(input + "\n"))
			if err != nil {
				fmt.Printf("Error writing to socket: %v\n", err)
			}

			response := make([]byte, 4096)
			n, err := conn.Read(response)
			if err != nil {
				fmt.Printf("Error reading server response: %v\n", err)
			}

			fmt.Println(string(response[:n]))
		}
	}

	if err := scanner.Err(); err != nil {
		fmt.Printf("Error closing scanner: %T", err)
	}
}
