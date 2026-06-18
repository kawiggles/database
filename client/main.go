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
	fmt.Print("Database prompt: ")

	for scanner.Scan() {
		payload := []byte(scanner.Text())
		fmt.Print("Database prompt: ")
		_, err := conn.Write(payload)
		if err != nil {
			fmt.Printf("Error writing to socket: %v\n", err)
		}
	}

}
