package main

var listCmd = &cobra.Command{
	Run: func(cmd *cobra.Command, args []string) {
		printDetails(provider) // line 5: call inside anonymous function
	},
}

func printDetails(p Provider) {
	// implementation
}
